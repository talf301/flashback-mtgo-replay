using FlashbackRecorder.Models;
using MTGOSDK.API;
using MTGOSDK.API.Collection;
using MTGOSDK.API.Play;
using MTGOSDK.API.Play.Games;
using static MTGOSDK.API.Events;

namespace FlashbackRecorder;

/// <summary>
/// MTGOSDK integration layer. Attaches to the MTGO process and translates
/// SDK events into the DTOs defined in <see cref="IMtgoClient"/>.
/// </summary>
public sealed class MtgoClient : IMtgoClient
{
    private static readonly TimeSpan ProcessPollInterval = TimeSpan.FromSeconds(3);

    private Client? _sdkClient;
    private bool _disposed;

    // ── Per-game state ──
    private Game? _currentGame;
    private Event? _currentEvent;
    private Dictionary<string, int> _previousLife = new();
    private Dictionary<string, CardCatalogEntry> _cardCatalog = new();

    public ConnectionState State { get; private set; } = ConnectionState.Disconnected;

    // ── Events ──

    public event EventHandler<ZoneChangeEventArgs>? OnZoneChange;
    public event EventHandler<GameActionEventArgs>? OnGameAction;
    public event EventHandler<LifeChangeEventArgs>? OnLifeChange;
    public event EventHandler<GamePhaseChangeEventArgs>? OnGamePhaseChange;
    public event EventHandler<TurnChangeEventArgs>? OnTurnChange;
    public event EventHandler<GameStatusChangeEventArgs>? OnGameStatusChange;

    // ── Connection lifecycle ──

    /// <summary>
    /// Attaches to a running MTGO process. If no process is found, polls at
    /// a fixed interval until one appears or cancellation is requested.
    /// </summary>
    public async Task ConnectAsync(CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);

        if (State == ConnectionState.Attached)
            return;

        State = ConnectionState.WaitingForProcess;

        while (!cancellationToken.IsCancellationRequested)
        {
            try
            {
                _sdkClient = new Client();
                SubscribeToEvents();

                State = ConnectionState.Attached;
                return;
            }
            catch (Exception) when (!cancellationToken.IsCancellationRequested)
            {
                // MTGO not running yet — wait and retry.
                _sdkClient?.Dispose();
                _sdkClient = null;

                await Task.Delay(ProcessPollInterval, cancellationToken)
                    .ConfigureAwait(false);
            }
        }

        cancellationToken.ThrowIfCancellationRequested();
    }

    public void Disconnect()
    {
        UnsubscribeFromEvents();
        _sdkClient?.Dispose();
        _sdkClient = null;
        _currentGame = null;
        _currentEvent = null;
        State = ConnectionState.Disconnected;
    }

    // ── Data providers ──

    /// <inheritdoc />
    public Dictionary<string, CardCatalogEntry> GetCardCatalog() => new(_cardCatalog);

    /// <inheritdoc />
    public DeckList? CaptureDeckList()
    {
        var deck = _currentEvent?.RegisteredDeck;
        if (deck == null) return null;

        return new DeckList
        {
            Mainboard = deck.GetCards(DeckRegion.MainDeck).Select(c => c.Name).ToList(),
            Sideboard = deck.GetCards(DeckRegion.Sideboard).Select(c => c.Name).ToList(),
        };
    }

    /// <inheritdoc />
    public Dictionary<string, object> CaptureSnapshot(int turn)
    {
        if (_currentGame == null)
            return new Dictionary<string, object>();

        var game = _currentGame;
        var players = new List<Dictionary<string, object>>();

        foreach (var player in game.Players)
        {
            var playerData = new Dictionary<string, object>
            {
                ["name"] = player.Name,
                ["seat"] = GetSeatIndex(game, player),
                ["life"] = player.Life,
            };

            // Mana pool
            var manaPool = new Dictionary<string, int>
            {
                ["W"] = 0, ["U"] = 0, ["B"] = 0, ["R"] = 0, ["G"] = 0, ["C"] = 0,
            };
            foreach (var mana in player.ManaPool)
            {
                var color = mana.Color.ToString();
                if (manaPool.ContainsKey(color))
                    manaPool[color]++;
                else
                    manaPool[color] = 1;
            }
            playerData["mana_pool"] = manaPool;

            // Zones
            var zones = new Dictionary<string, object>();
            zones["hand"] = CaptureZone(game, player, CardZone.Hand);
            zones["battlefield"] = CaptureZone(game, player, CardZone.Battlefield);
            zones["graveyard"] = CaptureZone(game, player, CardZone.Graveyard);
            zones["exile"] = CaptureZone(game, player, CardZone.Exile);
            zones["library"] = new Dictionary<string, object>
            {
                ["cards"] = new List<object>(),
                ["count"] = player.LibraryCount,
            };
            playerData["zones"] = zones;

            players.Add(playerData);
        }

        var snapshot = new Dictionary<string, object>
        {
            ["players"] = players,
        };

        if (game.ActivePlayer != null)
            snapshot["active_player"] = game.ActivePlayer.Name;
        if (game.PriorityPlayer != null)
            snapshot["priority_player"] = game.PriorityPlayer.Name;

        return snapshot;
    }

    private Dictionary<string, object> CaptureZone(Game game, GamePlayer player, CardZone zoneType)
    {
        var zone = game.GetGameZone(player, zoneType);
        var cards = new List<Dictionary<string, object>>();

        if (zone != null)
        {
            foreach (var card in zone.Cards)
            {
                TryCaptureCardMetadata(card);

                var cardData = new Dictionary<string, object>
                {
                    ["id"] = card.Id.ToString(),
                    ["catalog_id"] = card.Id.ToString(),
                    ["tapped"] = card.IsTapped,
                    ["face_down"] = card.IsFlipped,
                    ["summoning_sickness"] = card.HasSummoningSickness,
                };

                if (card.Power != null) cardData["power"] = card.Power;
                if (card.Toughness != null) cardData["toughness"] = card.Toughness;
                cardData["damage"] = card.Damage;

                // Counters
                var counters = card.Counters
                    .GroupBy(c => c)
                    .ToDictionary(g => g.Key.ToString(), g => g.Count());
                if (counters.Count > 0)
                    cardData["counters"] = counters;

                // Attachments
                var attachments = card.Associations
                    .Where(a => a.Type == CardAssociation.EquippedTo || a.Type == CardAssociation.EquippedWith)
                    .Select(a => a.Card.Id.ToString())
                    .ToList();
                if (attachments.Count > 0)
                    cardData["attachments"] = attachments;

                // Combat status
                if (card.IsAttacking || card.IsBlocking)
                {
                    var combat = new Dictionary<string, object>();
                    if (card.IsAttacking)
                    {
                        combat["attacking"] = true;
                        var targets = card.AttackingOrders?.Select(p => p.Name).ToList();
                        if (targets?.Count > 0) combat["target"] = targets[0];
                    }
                    if (card.IsBlocking)
                    {
                        combat["blocking"] = true;
                        var blocked = card.BlockingOrders?.Select(c => c.Id.ToString()).ToList();
                        if (blocked?.Count > 0) combat["blocked"] = blocked;
                    }
                    cardData["combat_status"] = combat;
                }

                // Controller (only if different from owner)
                if (card.Controller?.Name != card.Owner?.Name && card.Controller != null)
                    cardData["controller"] = card.Controller.Name;

                cards.Add(cardData);
            }
        }

        var result = new Dictionary<string, object> { ["cards"] = cards };
        if (zoneType == CardZone.Hand)
            result["count"] = cards.Count;
        return result;
    }

    // ── Card catalog ──

    private void TryCaptureCardMetadata(GameCard card)
    {
        var id = card.Id.ToString();
        if (_cardCatalog.ContainsKey(id)) return;

        var def = card.Definition;
        _cardCatalog[id] = new CardCatalogEntry
        {
            Name = card.Name,
            ManaCost = def?.ManaCost,
            TypeLine = def != null
                ? string.Join(" ", def.Types) + (def.Subtypes.Count > 0
                    ? " \u2014 " + string.Join(" ", def.Subtypes)
                    : "")
                : null,
        };
    }

    // ── SDK event wiring ──

    private void SubscribeToEvents()
    {
        EventManager.GameJoined += OnSdkGameJoined;
    }

    private void UnsubscribeFromEvents()
    {
        EventManager.GameJoined -= OnSdkGameJoined;
    }

    /// <summary>
    /// Returns a seat index for a player by finding their position in the
    /// game's player list. Returns -1 if not found.
    /// </summary>
    private static int GetSeatIndex(Game game, GamePlayer player)
    {
        var players = game.Players;
        for (int i = 0; i < players.Count; i++)
        {
            if (players[i].Name == player.Name)
                return i;
        }
        return -1;
    }

    /// <summary>
    /// Called when the local player joins a game. Sets up per-game event
    /// subscriptions on the <see cref="Game"/> instance.
    /// </summary>
    private void OnSdkGameJoined(Event playerEvent, Game game)
    {
        _currentGame = game;
        _currentEvent = playerEvent;
        _cardCatalog.Clear();

        // Initialize life cache from game players.
        _previousLife.Clear();
        foreach (var player in game.Players)
        {
            _previousLife[player.Name] = player.Life;
        }

        // Notify that a new game has started with player and format info.
        OnGameStatusChange?.Invoke(this, new GameStatusChangeEventArgs
        {
            Status = GameStatus.Started,
            GameId = game.Id,
            Players = game.Players.Select((p, i) => new PlayerInfo
            {
                Name = p.Name,
                Seat = i,
            }).ToList(),
            Format = playerEvent.Format?.Name,
        });

        // Subscribe to per-game events.
        SubscribeToGameEvents(game);
    }

    private void SubscribeToGameEvents(Game game)
    {
        game.OnZoneChange += (GameCard card) =>
        {
            TryCaptureCardMetadata(card);

            OnZoneChange?.Invoke(this, new ZoneChangeEventArgs
            {
                CardId = card.Id,
                CardName = card.Name,
                SourceZone = card.PreviousZone?.ToString() ?? "Unknown",
                DestinationZone = card.Zone?.ToString() ?? "Unknown",
                OwnerSeat = GetSeatIndex(game, card.Owner),
            });
        };

        game.OnGameAction += (GameAction action) =>
        {
            var cardAction = action as CardAction;
            if (cardAction?.Card != null)
                TryCaptureCardMetadata(cardAction.Card);

            OnGameAction?.Invoke(this, new GameActionEventArgs
            {
                ActionType = action.Type.ToString(),
                CardId = cardAction?.Card?.Id ?? action.ActionId,
                CardName = cardAction?.Card?.Name ?? action.Name,
                PlayerSeat = cardAction?.Card != null
                    ? GetSeatIndex(game, cardAction.Card.Controller)
                    : 0,
                AbilityText = action.Name,
                SourceZone = cardAction?.Card?.Zone?.ToString(),
            });
        };

        game.OnLifeChange += (GamePlayer player) =>
        {
            _previousLife.TryGetValue(player.Name, out var oldLife);
            OnLifeChange?.Invoke(this, new LifeChangeEventArgs
            {
                PlayerSeat = GetSeatIndex(game, player),
                OldLife = oldLife,
                NewLife = player.Life,
                Source = null,  // SDK doesn't expose life change source
            });
            _previousLife[player.Name] = player.Life;
        };

        game.OnGamePhaseChange += (CurrentPlayerPhase phase) =>
        {
            OnGamePhaseChange?.Invoke(this, new GamePhaseChangeEventArgs
            {
                Phase = phase.CurrentPhase.ToString(),
                ActivePlayerSeat = GetSeatIndex(game, phase.ActivePlayer),
            });
        };

        game.CurrentTurnChanged += (GameEventArgs args) =>
        {
            OnTurnChange?.Invoke(this, new TurnChangeEventArgs
            {
                TurnNumber = game.CurrentTurn,
                ActivePlayerSeat = GetSeatIndex(game, game.ActivePlayer),
                ActivePlayerName = game.ActivePlayer?.Name ?? "",
            });
        };

        game.GameStatusChanged += (GameStatusEventArgs args) =>
        {
            if (args.NewStatus == MTGOSDK.API.Play.Games.GameStatus.Finished)
            {
                var winners = game.WinningPlayers;
                OnGameStatusChange?.Invoke(this, new GameStatusChangeEventArgs
                {
                    Status = FlashbackRecorder.GameStatus.Ended,
                    GameId = game.Id,
                    WinnerName = winners.Count > 0 ? winners[0].Name : null,
                    Reason = null,
                });
            }
        };
    }

    // ── IDisposable ──

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Disconnect();
    }
}
