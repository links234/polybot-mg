# Analyze Command Text Filtering

## Overview
The `analyze` command now supports advanced text filtering capabilities for market data analysis.

## Implemented Filters

### Title Filtering
- `--title-contains <text>` - Filter markets where title contains the specified text (case-insensitive)
- `--title-contains-any <keywords>` - Filter markets where title contains ANY of the comma-separated keywords
- `--title-contains-all <keywords>` - Filter markets where title contains ALL of the comma-separated keywords
- `--title-regex <pattern>` - Filter markets where title matches the pattern (currently simple contains)

### Content Filtering
- `--description-contains <text>` - Filter markets where description contains the specified text
- `--text-search <text>` - Search across title, description, and tags simultaneously
- `--fuzzy-search <text>` - Fuzzy text matching in title and description
- `--fuzzy-threshold <0.0-1.0>` - Set the similarity threshold for fuzzy search (default: 0.7)

### Existing Filters
- `--categories <categories>` - Filter by comma-separated categories
- `--tags <tags>` - Filter by comma-separated tags
- `--min-price <0-100>` - Minimum price for YES outcome
- `--max-price <0-100>` - Maximum price for YES outcome
- `--active-only` - Only active markets
- `--accepting-orders-only` - Only markets accepting orders
- `--no-archived` - Exclude archived markets
- `--ending-before <date>` - Markets ending before specified date

## Usage Examples

### Find Bitcoin Price Prediction Markets
```bash
polybot analyze bitcoin_bets \
  --source-dataset raw_markets \
  --title-contains bitcoin \
  --title-contains-any "price,up,down,above,below,bull,bear" \
  --active-only
```

### Search Election Markets
```bash
polybot analyze election_markets \
  --source-dataset raw_markets \
  --text-search election \
  --categories Politics,Elections \
  --fuzzy-search "president vote" \
  --fuzzy-threshold 0.8
```

### Complex Multi-Filter Query
```bash
polybot analyze filtered_markets \
  --source-dataset raw_markets \
  --title-contains-all "climate,temperature" \
  --description-contains "global warming" \
  --tags Environment,Science \
  --min-price 20 \
  --max-price 80 \
  --active-only
```

## Implementation Notes

1. **Case Insensitive**: All text searches are case-insensitive
2. **Title Field**: The filter checks both `question` and `title` fields for compatibility
3. **Fuzzy Search**: Uses character matching and word presence for similarity scoring
4. **Performance**: Filters are applied during the initial data loading phase for efficiency

## Future Enhancements

- Add proper regex support with the regex crate
- Implement relevance scoring for search results
- Add stemming and lemmatization for better text matching
- Support for negative filters (NOT contains)
- Add synonym matching for improved search 