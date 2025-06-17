# Architecture Documentation

This section contains system design documentation and architectural decisions.

## 🏗️ System Design

### [Data Structure](./data-structure.md)
Core data models and relationships:
- Entity definitions and schemas
- Data flow diagrams
- Storage patterns
- API interfaces

## 📋 Architecture Overview

Polybot follows a modular architecture with clear separation of concerns:

### Core Components

1. **CLI Layer** (`src/cli/`) - Command-line interface and argument parsing
2. **TUI Layer** (`src/tui/`) - Terminal user interface components
3. **WebSocket Client** (`src/ws/`) - Real-time data streaming
4. **Services** (`src/services/`) - Background services and orchestration
5. **Storage** (`src/storage/`) - Data persistence and discovery
6. **Datasets** (`src/datasets/`) - Dataset management and selection
7. **Markets** (`src/markets/`) - Market data processing
8. **Execution** (`src/execution/`) - Trading execution engine

### Design Principles

- **Async-First** - Built on Tokio async runtime
- **Type Safety** - Comprehensive use of Rust's type system
- **Error Handling** - Robust error propagation with context
- **Modularity** - Clear separation between components
- **Testability** - Unit tests with `#[cfg(test)]` modules

### Data Flow

```
CLI Input → Command Processing → Service Layer → WebSocket/API → Data Processing → Storage/Display
```

## 🔗 Related Documentation

- **[Features](../features/)** - Implementation details for specific features
- **[Development](../development/)** - Development and testing guides
- **[Troubleshooting](../troubleshooting/)** - Common issues and solutions

---

[← Back to Documentation Index](../README.md)