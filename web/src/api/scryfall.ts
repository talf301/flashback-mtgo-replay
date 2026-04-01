/**
 * Scryfall API Integration
 *
 * Provides card data retrieval with caching for improved performance.
 * Uses the Scryfall REST API to fetch card information.
 */

export interface ScryfallCard {
  id: string;
  name: string;
  cmc: number;
  type_line: string;
  oracle_text?: string;
  mana_cost?: string;
  colors: string[];
  color_identity: string[];
  image_uris: {
    small: string;
    normal: string;
    large: string;
    png: string;
    art_crop: string;
    border_crop: string;
  };
  legalities: Record<string, string>;
  set_name: string;
  collector_number: string;
  power?: string;
  toughness?: string;
  loyalty?: string;
}

export interface ScryfallError {
  object: 'error';
  code: string;
  status: number;
  details: string;
  type?: string;
  warnings?: string[];
}

/**
 * In-memory card data cache
 * Key: card name or Scryfall ID
 * Value: ScryfallCard data
 */
const cardCache = new Map<string, ScryfallCard>();

/**
 * In-memory cache for search queries
 * Key: query string
 * Value: Array of ScryfallCard
 */
const searchCache = new Map<string, ScryfallCard[]>();

/**
 * Fetches card data from Scryfall API
 *
 * @param identifier - Card name, set/collector number, or Scryfall UUID
 * @returns Promise<ScryfallCard> - Card data
 * @throws Error if card not found or API error occurs
 */
export async function getCardData(identifier: string): Promise<ScryfallCard> {
  // Check cache first
  const cached = cardCache.get(identifier.toLowerCase());
  if (cached) {
    return cached;
  }

  try {
    const response = await fetch(`https://api.scryfall.com/cards/named?fuzzy=${encodeURIComponent(identifier)}`);

    if (!response.ok) {
      const error: ScryfallError = await response.json();
      throw new Error(`Scryfall API error (${error.status}): ${error.details}`);
    }

    const card: ScryfallCard = await response.json();

    // Cache the card
    cacheCardData(identifier, card);

    return card;
  } catch (error) {
    if (error instanceof Error) {
      throw new Error(`Failed to fetch card data: ${error.message}`);
    }
    throw new Error('Unknown error fetching card data');
  }
}

/**
 * Fetches multiple cards by their Scryfall IDs
 *
 * @param ids - Array of Scryfall card IDs
 * @returns Promise<ScryfallCard[]> - Array of card data
 * @throws Error if any card not found or API error occurs
 */
export async function getCardBatch(ids: string[]): Promise<ScryfallCard[]> {
  const uncachedIds: string[] = [];
  const results: ScryfallCard[] = [];

  // Check cache for each ID
  for (const id of ids) {
    const cached = cardCache.get(id);
    if (cached) {
      results.push(cached);
    } else {
      uncachedIds.push(id);
    }
  }

  // Batch fetch uncached cards (Scryfall supports up to 75 per request)
  if (uncachedIds.length > 0) {
    try {
      const response = await fetch('https://api.scryfall.com/cards/collection', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          identifiers: uncachedIds.map((id) => ({ id })),
        }),
      });

      if (!response.ok) {
        const error: ScryfallError = await response.json();
        throw new Error(`Scryfall API error (${error.status}): ${error.details}`);
      }

      const data = await response.json();

      if (!data.data) {
        throw new Error('No data returned from Scryfall API');
      }

      // Cache and collect the fetched cards
      for (const card of data.data) {
        cacheCardData(card.id, card);
        results.push(card);
      }
    } catch (error) {
      if (error instanceof Error) {
        throw new Error(`Failed to fetch card batch: ${error.message}`);
      }
      throw new Error('Unknown error fetching card batch');
    }
  }

  return results;
}

/**
 * Searches for cards using a Scryfall query
 *
 * @param query - Scryfall search query
 * @returns Promise<ScryfallCard[]> - Array of matching cards
 * @throws Error if search fails
 */
export async function searchCards(query: string): Promise<ScryfallCard[]> {
  // Check cache
  const cached = searchCache.get(query);
  if (cached) {
    return cached;
  }

  try {
    const response = await fetch(`https://api.scryfall.com/cards/search?q=${encodeURIComponent(query)}`);

    if (!response.ok) {
      const error: ScryfallError = await response.json();
      throw new Error(`Scryfall API error (${error.status}): ${error.details}`);
    }

    const data = await response.json();

    if (!data.data) {
      return [];
    }

    const cards = data.data as ScryfallCard[];

    // Cache the search results
    searchCache.set(query, cards);

    return cards;
  } catch (error) {
    if (error instanceof Error) {
      throw new Error(`Failed to search cards: ${error.message}`);
    }
    throw new Error('Unknown error searching cards');
  }
}

/**
 * Caches card data in memory
 *
 * @param key - Cache key (card name, ID, etc.)
 * @param card - Card data to cache
 */
export function cacheCardData(key: string, card: ScryfallCard): void {
  const cacheKey = key.toLowerCase();
  cardCache.set(cacheKey, card);

  // Also cache by ID and name for flexible lookups
  cardCache.set(card.id, card);
  cardCache.set(card.name.toLowerCase(), card);
}

/**
 * Clears all card data from cache
 */
export function clearCardCache(): void {
  cardCache.clear();
  searchCache.clear();
}

/**
 * Gets cache statistics
 *
 * @returns Object with cache hit counts and sizes
 */
export function getCacheStats(): {
  cardCacheSize: number;
  searchCacheSize: number;
} {
  return {
    cardCacheSize: cardCache.size,
    searchCacheSize: searchCache.size,
  };
}
