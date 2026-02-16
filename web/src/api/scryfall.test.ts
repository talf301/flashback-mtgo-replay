/**
 * Tests for Scryfall API Integration
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  getCardData,
  getCardBatch,
  searchCards,
  cacheCardData,
  clearCardCache,
  getCacheStats,
  type ScryfallCard,
} from './scryfall';

// Mock fetch
const mockFetch = vi.fn();
global.fetch = mockFetch as any;

describe('Scryfall API', () => {
  beforeEach(() => {
    clearCardCache();
    mockFetch.mockClear();
  });

  describe('getCardData', () => {
    it('should fetch and cache card data', async () => {
      const mockCard: ScryfallCard = {
        id: 'test-id-123',
        name: 'Lightning Bolt',
        cmc: 1,
        type_line: 'Instant',
        oracle_text: 'Lightning Bolt deals 3 damage to any target.',
        mana_cost: '{R}',
        colors: ['R'],
        color_identity: ['R'],
        image_uris: {
          small: 'https://example.com/small.jpg',
          normal: 'https://example.com/normal.jpg',
          large: 'https://example.com/large.jpg',
          png: 'https://example.com/card.png',
          art_crop: 'https://example.com/art.jpg',
          border_crop: 'https://example.com/border.jpg',
        },
        legalities: { standard: 'legal', modern: 'legal' },
        set_name: 'Magic 2010',
        collector_number: '123',
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockCard,
      });

      const result = await getCardData('Lightning Bolt');

      expect(result).toEqual(mockCard);
      expect(mockFetch).toHaveBeenCalledWith(
        'https://api.scryfall.com/cards/named?fuzzy=Lightning%20Bolt',
      );

      // Second call should use cache
      const cached = await getCardData('Lightning Bolt');
      expect(cached).toEqual(mockCard);
      expect(mockFetch).toHaveBeenCalledTimes(1);
    });

    it('should handle API errors', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 404,
        json: async () => ({
          object: 'error',
          code: 'not_found',
          status: 404,
          details: 'Card not found',
        }),
      });

      await expect(getCardData('Nonexistent Card')).rejects.toThrow(
        'Scryfall API error (404): Card not found',
      );
    });

    it('should be case-insensitive and use cache', async () => {
      const mockCard: ScryfallCard = {
        id: 'test-id-456',
        name: 'Black Lotus',
        cmc: 0,
        type_line: 'Artifact',
        colors: [],
        color_identity: [],
        image_uris: {
          small: 'https://example.com/small.jpg',
          normal: 'https://example.com/normal.jpg',
          large: 'https://example.com/large.jpg',
          png: 'https://example.com/card.png',
          art_crop: 'https://example.com/art.jpg',
          border_crop: 'https://example.com/border.jpg',
        },
        legalities: { standard: 'not_legal' },
        set_name: 'Limited Edition Alpha',
        collector_number: '232',
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockCard,
      });

      await getCardData('black lotus');
      const cached = await getCardData('Black Lotus');
      const cached2 = await getCardData('BLACK LOTUS');

      expect(mockFetch).toHaveBeenCalledTimes(1);
      expect(cached).toEqual(mockCard);
      expect(cached2).toEqual(mockCard);
    });
  });

  describe('getCardBatch', () => {
    it('should fetch multiple cards and cache them', async () => {
      const mockCards: ScryfallCard[] = [
        {
          id: 'card-1',
          name: 'Card One',
          cmc: 2,
          type_line: 'Creature',
          colors: ['U'],
          color_identity: ['U'],
          image_uris: {
            small: 'https://example.com/small.jpg',
            normal: 'https://example.com/normal.jpg',
            large: 'https://example.com/large.jpg',
            png: 'https://example.com/card.png',
            art_crop: 'https://example.com/art.jpg',
            border_crop: 'https://example.com/border.jpg',
          },
          legalities: {},
          set_name: 'Test',
          collector_number: '1',
        },
        {
          id: 'card-2',
          name: 'Card Two',
          cmc: 3,
          type_line: 'Sorcery',
          colors: ['R'],
          color_identity: ['R'],
          image_uris: {
            small: 'https://example.com/small.jpg',
            normal: 'https://example.com/normal.jpg',
            large: 'https://example.com/large.jpg',
            png: 'https://example.com/card.png',
            art_crop: 'https://example.com/art.jpg',
            border_crop: 'https://example.com/border.jpg',
          },
          legalities: {},
          set_name: 'Test',
          collector_number: '2',
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ data: mockCards }),
      });

      const results = await getCardBatch(['card-1', 'card-2']);

      expect(results).toHaveLength(2);
      expect(results[0].id).toBe('card-1');
      expect(results[1].id).toBe('card-2');

      // Second call should use cache
      const cached = await getCardBatch(['card-1', 'card-2']);
      expect(cached).toEqual(results);
      expect(mockFetch).toHaveBeenCalledTimes(1);
    });

    it('should use cached cards and only fetch uncached', async () => {
      const mockCard1: ScryfallCard = {
        id: 'card-1',
        name: 'Card One',
        cmc: 2,
        type_line: 'Creature',
        colors: ['U'],
        color_identity: ['U'],
        image_uris: {
          small: 'https://example.com/small.jpg',
          normal: 'https://example.com/normal.jpg',
          large: 'https://example.com/large.jpg',
          png: 'https://example.com/card.png',
          art_crop: 'https://example.com/art.jpg',
          border_crop: 'https://example.com/border.jpg',
        },
        legalities: {},
        set_name: 'Test',
        collector_number: '1',
      };

      const mockCard2: ScryfallCard = {
        id: 'card-2',
        name: 'Card Two',
        cmc: 3,
        type_line: 'Sorcery',
        colors: ['R'],
        color_identity: ['R'],
        image_uris: {
          small: 'https://example.com/small.jpg',
          normal: 'https://example.com/normal.jpg',
          large: 'https://example.com/large.jpg',
          png: 'https://example.com/card.png',
          art_crop: 'https://example.com/art.jpg',
          border_crop: 'https://example.com/border.jpg',
        },
        legalities: {},
        set_name: 'Test',
        collector_number: '2',
      };

      // Pre-cache card-1
      cacheCardData('card-1', mockCard1);

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ data: [mockCard2] }),
      });

      const results = await getCardBatch(['card-1', 'card-2']);

      expect(results).toHaveLength(2);
      expect(mockFetch).toHaveBeenCalledTimes(1);
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('cards/collection'),
        expect.objectContaining({
          body: expect.stringContaining('"id":"card-2"'),
        }),
      );
    });

    it('should handle empty result set', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ data: [] }),
      });

      const results = await getCardBatch(['card-1', 'card-2']);
      expect(results).toHaveLength(0);
    });
  });

  describe('searchCards', () => {
    it('should search for cards and cache results', async () => {
      const mockCards: ScryfallCard[] = [
        {
          id: 'search-1',
          name: 'Search Card 1',
          cmc: 1,
          type_line: 'Instant',
          colors: ['W'],
          color_identity: ['W'],
          image_uris: {
            small: 'https://example.com/small.jpg',
            normal: 'https://example.com/normal.jpg',
            large: 'https://example.com/large.jpg',
            png: 'https://example.com/card.png',
            art_crop: 'https://example.com/art.jpg',
            border_crop: 'https://example.com/border.jpg',
          },
          legalities: {},
          set_name: 'Test',
          collector_number: '1',
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ data: mockCards }),
      });

      const results = await searchCards('type:instant cmc=1');

      expect(results).toHaveLength(1);
      expect(results[0].name).toBe('Search Card 1');

      // Second call should use cache
      const cached = await searchCards('type:instant cmc=1');
      expect(cached).toEqual(results);
      expect(mockFetch).toHaveBeenCalledTimes(1);
    });

    it('should return empty array for no results', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ data: [] }),
      });

      const results = await searchCards('name:"Card That Does Not Exist"');
      expect(results).toHaveLength(0);
    });

    it('should handle search errors', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 400,
        json: async () => ({
          object: 'error',
          code: 'bad_request',
          status: 400,
          details: 'Invalid search query',
        }),
      });

      await expect(searchCards('invalid query!')).rejects.toThrow(
        'Scryfall API error (400): Invalid search query',
      );
    });
  });

  describe('cacheCardData', () => {
    it('should cache card data with multiple keys', () => {
      const mockCard: ScryfallCard = {
        id: 'cache-test-123',
        name: 'Cache Test Card',
        cmc: 2,
        type_line: 'Creature',
        colors: ['G'],
        color_identity: ['G'],
        image_uris: {
          small: 'https://example.com/small.jpg',
          normal: 'https://example.com/normal.jpg',
          large: 'https://example.com/large.jpg',
          png: 'https://example.com/card.png',
          art_crop: 'https://example.com/art.jpg',
          border_crop: 'https://example.com/border.jpg',
        },
        legalities: {},
        set_name: 'Test',
        collector_number: '1',
      };

      cacheCardData('test-key', mockCard);

      const stats = getCacheStats();
      expect(stats.cardCacheSize).toBeGreaterThan(0);
    });
  });

  describe('clearCardCache', () => {
    it('should clear all caches', async () => {
      const mockCard: ScryfallCard = {
        id: 'clear-test-123',
        name: 'Clear Test Card',
        cmc: 2,
        type_line: 'Creature',
        colors: ['B'],
        color_identity: ['B'],
        image_uris: {
          small: 'https://example.com/small.jpg',
          normal: 'https://example.com/normal.jpg',
          large: 'https://example.com/large.jpg',
          png: 'https://example.com/card.png',
          art_crop: 'https://example.com/art.jpg',
          border_crop: 'https://example.com/border.jpg',
        },
        legalities: {},
        set_name: 'Test',
        collector_number: '1',
      };

      cacheCardData('test-key', mockCard);
      expect(getCacheStats().cardCacheSize).toBeGreaterThan(0);

      clearCardCache();
      expect(getCacheStats().cardCacheSize).toBe(0);
      expect(getCacheStats().searchCacheSize).toBe(0);
    });
  });

  describe('getCacheStats', () => {
    it('should return accurate cache statistics', () => {
      clearCardCache();

      const stats = getCacheStats();
      expect(stats.cardCacheSize).toBe(0);
      expect(stats.searchCacheSize).toBe(0);

      const mockCard: ScryfallCard = {
        id: 'stats-test-123',
        name: 'Stats Test Card',
        cmc: 2,
        type_line: 'Creature',
        colors: ['R'],
        color_identity: ['R'],
        image_uris: {
          small: 'https://example.com/small.jpg',
          normal: 'https://example.com/normal.jpg',
          large: 'https://example.com/large.jpg',
          png: 'https://example.com/card.png',
          art_crop: 'https://example.com/art.jpg',
          border_crop: 'https://example.com/border.jpg',
        },
        legalities: {},
        set_name: 'Test',
        collector_number: '1',
      };

      cacheCardData('test-1', mockCard);
      cacheCardData('test-2', mockCard);

      const statsAfter = getCacheStats();
      expect(statsAfter.cardCacheSize).toBeGreaterThan(0);
    });
  });
});
