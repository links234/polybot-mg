# Polybot Pipeline Configurations

This directory contains YAML pipeline configurations for automated market data processing and analysis workflows.

## Available Pipelines

### 1. fetch_raw_markets.yaml
**Purpose**: Fetch and cache all raw markets data for use by other pipelines.
- Creates a shared dataset at `raw_markets/YYYY-MM-DD/`
- Caches data for 6 hours by default
- Used as the data source for all other pipelines

### 2. quick_fetch.yaml
**Purpose**: Rapidly analyze active markets from shared raw market data.
- Filters for active markets only
- Creates datasets at `quick_active/YYYY-MM-DD/timestamp/`
- Lightweight analysis for quick market overview

### 3. market_analysis.yaml
**Purpose**: Comprehensive market analysis with filtering, enrichment, and reporting.
- Multi-step pipeline with progressive filtering
- Enriches markets with metadata
- Generates detailed HTML reports
- Creates datasets at `active_markets/`, `enriched_markets/`, and `market_analysis_report/` folders

### 4. bitcoin_markets.yaml
**Purpose**: Filter Bitcoin-related markets from shared raw market data.
- Filters for active Bitcoin markets
- Price range filtering (1-99%)
- Generates Bitcoin-specific reports
- Creates datasets at `bitcoin_markets/YYYY-MM-DD/timestamp/`

### 5. bitcoin_price_bets.yaml
**Purpose**: Filter markets specifically for Bitcoin up/down price predictions.
- Uses `--title-contains` to find markets with "bitcoin" in the title
- Uses `--title-contains-any` to match price-related keywords
- Filters active binary markets (price range 1-99%)
- Creates datasets at `bitcoin_price_bets/YYYY-MM-DD/timestamp/`

### 6. election_markets.yaml
**Purpose**: Filter and analyze election-related prediction markets.
- Demonstrates advanced text filtering:
  - `--text-search`: Searches across title, description, and tags
  - `--fuzzy-search`: Fuzzy matching with configurable threshold
  - `--categories`: Filter by market categories
  - `--ending-before`: Date-based filtering
- Creates datasets at `election_markets/YYYY-MM-DD/timestamp/`

### 7. daily_monitor.yaml
**Purpose**: Daily monitoring of trending and high-volume markets.
- Identifies trending markets
- Filters high-volume markets (>$1,000)
- Generates daily monitoring reports
- Creates datasets at `daily_trending/` and `daily_high_volume/` folders

### 8. high_value_analysis.yaml
**Purpose**: Deep analysis of high-value markets with enrichment.
- Filters markets with >$10,000 volume
- Enriches with orderbook and trade data
- Generates trading signals (arbitrage, momentum)
- Creates comprehensive reports with HTML output
- Creates datasets at `high_value_markets/` and `high_value_enriched/` folders

## Advanced Filtering Options

The `analyze` command now supports powerful text filtering:

### Title Filtering
- `--title-contains <text>`: Market title must contain this exact text (case-insensitive)
- `--title-contains-any <keywords>`: Title must contain ANY of these comma-separated keywords
- `--title-contains-all <keywords>`: Title must contain ALL of these comma-separated keywords
- `--title-regex <pattern>`: Title matches pattern (currently simple contains matching)

### Content Filtering
- `--description-contains <text>`: Description must contain this text
- `--text-search <text>`: Search across title, description, and tags
- `--fuzzy-search <text>`: Fuzzy text matching with threshold
- `--fuzzy-threshold <0.0-1.0>`: Similarity threshold for fuzzy search (default: 0.7)

### Example Usage in Pipelines
```yaml
# Find markets about specific topics
args:
  - "--title-contains"
  - "bitcoin"
  - "--title-contains-any"
  - "price,up,down,above,below"

# Complex multi-field search
args:
  - "--text-search"
  - "climate"
  - "--categories"
  - "Science,Environment"
  - "--description-contains"
  - "temperature"
```

## Dataset Organization

All data files use a date-based folder structure in the main datasets directory:
```
./data/datasets/
  raw_markets/
    2025-06-10/           # Shared raw data, cached for reuse
  bitcoin_markets/
    2025-06-10/
      2025-06-10_14-30-45/  # Timestamp-specific results
  bitcoin_price_bets/
    2025-06-10/
      2025-06-10_14-35-20/
  election_markets/
    2025-06-10/
      2025-06-10_15-45-00/
  ... etc
  
  runs/                   # Pipeline execution metadata only
    pipeline_*_*/         # Contains dataset.yaml tracking pipeline execution
```

Command outputs and analysis results are saved in `./data/datasets/` organized by type and date.
Pipeline execution metadata is saved in `./data/datasets/runs/` for tracking workflow executions.

## Running Pipelines

To run a pipeline:
```bash
polybot pipeline run <pipeline_name>

# Examples:
polybot pipeline run fetch_raw_markets
polybot pipeline run bitcoin_price_bets
polybot pipeline run election_markets
polybot pipeline run high_value_analysis
```

## Pipeline Parameters

Pipelines support parameter overrides:
```bash
polybot pipeline run bitcoin_markets --param min_price=10 --param max_price=90
polybot pipeline run election_markets --param fuzzy_threshold=0.8
```

## Caching Strategy

- Raw market data is cached for 6 hours by default
- All analysis pipelines reuse the cached raw data
- Force refresh with: `--param force_refresh=true`
- Cache duration can be adjusted with: `--param cache_duration=12`

## Future Enhancements

1. **Regex Support**: Add proper regex pattern matching when regex crate is added
2. **Volume Filtering**: Add `--min-volume-usd` option to filter by market volume
3. **Advanced Scoring**: Implement relevance scoring for search results
4. **Natural Language Search**: Add NLP-based search capabilities