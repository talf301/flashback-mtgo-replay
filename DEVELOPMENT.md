# Development Guide

This guide covers development practices, testing, and code style for the Flashback MTGO Replay System.

## Prerequisites

- Rust 1.70+
- Node.js 18+
- npm

## Project Setup

```bash
# Clone repository
git clone <repository-url>
cd mtgo-replay-omp

# Install Rust dependencies (if needed)
cargo build

# Install web dependencies
cd web
npm install
cd ..
```

## Development Workflow

### Rust (Capture Agent)

```bash
# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_replay_file_serialization

# Run with clippy (linter)
cargo clippy -- -D warnings

# Format code
cargo fmt

# Build for development
cargo build

# Build release binary
cargo build --release
```

### Web (TypeScript + React)

```bash
cd web

# Start development server (with hot reload)
npm run dev

# Run tests
npm test

# Run tests in watch mode
npm test -- --watch

# Run type checking
npm run type-check

# Lint code
npm run lint

# Build for production
npm run build

# Preview production build
npm run preview
```

## Testing

### Rust Tests

Tests are organized by module:

```rust
// Unit tests in module files
#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {
        // ...
    }
}

// Integration tests in tests/ directory
// See tests/integration.rs for examples
```

Run all tests:
```bash
cargo test --all
```

Run specific test file:
```bash
cargo test --test integration
```

### Web Tests

Tests use Vitest with React Testing Library:

```typescript
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { App } from './App';

describe('App Component', () => {
  it('renders without crashing', () => {
    render(<App />);
    expect(screen.getByText('MTG Replay Viewer')).toBeInTheDocument();
  });
});
```

## Code Style

### Rust

- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Follow Rust naming conventions:
  - Functions and methods: `snake_case`
  - Types and traits: `PascalCase`
  - Constants: `SCREAMING_SNAKE_CASE`
- Document public APIs with `///` comments
- Use `Result<T, E>` for fallible operations
- Prefer `thiserror` for custom error types

### TypeScript

- Use Prettier for formatting (configured in `.prettierrc`)
- Use ESLint for linting
- Follow TypeScript naming conventions:
  - Variables and functions: `camelCase`
  - Classes and interfaces: `PascalCase`
  - Constants: `UPPER_SNAKE_CASE`
- Use functional components with hooks
- Document complex components with JSDoc comments

## Commit Convention

Follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` - New feature
- `fix:` - Bug fix
- `test:` - Adding or updating tests
- `docs:` - Documentation changes
- `refactor:` - Code refactoring
- `chore:` - Maintenance tasks

Examples:
```
feat: add replay export functionality
fix: resolve issue with card image loading
test: add integration test for full pipeline
docs: update README with new features
refactor: simplify state management in App component
chore: update dependencies
```

## Project-Specific Guidelines

### Error Handling (Rust)

Use the `ReplayError` type for all replay-related errors:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Replay file not found: {0}")]
    NotFound(String),
    #[error("Invalid replay format: {0}")]
    InvalidFormat(String),
}

pub type Result<T> = std::result::Result<T, ReplayError>;
```

### State Management (Web)

Use React hooks for state management:

```typescript
import { useState, useEffect, useCallback } from 'react';

function MyComponent() {
  const [data, setData] = useState<DataType | null>(null);
  const [loading, setLoading] = useState(false);

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const result = await apiCall();
      setData(result);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  // ...
}
```

### Type Safety

- Enable strict type checking in `tsconfig.json`
- Avoid `any` types - use `unknown` when necessary
- Define interfaces for all data structures

## Adding New Features

### 1. Plan the Feature

- Define the requirements
- Identify affected components
- Consider impact on existing functionality

### 2. Implement

- Write tests first (TDD recommended)
- Implement the feature
- Update documentation

### 3. Test

- Run unit tests
- Run integration tests
- Manual testing if needed

### 4. Code Review

- Self-review your changes
- Ensure code follows style guidelines
- Update documentation as needed

## Troubleshooting

### Common Issues

**Build fails with "unresolved imports"**
- Run `cargo clean` and rebuild
- Check that all dependencies are in `Cargo.toml`

**Tests fail intermittently**
- Check for timing issues
- Ensure proper cleanup in test `afterEach` hooks
- Use deterministic test data

**Web viewer won't load replays**
- Check browser console for errors
- Verify replay file format matches schema
- Check API endpoint configuration

### Debugging Tips

**Rust**
```bash
# Run with debug output
RUST_LOG=debug cargo run

# Use rust-gdb for debugging
rust-gdb target/debug/flashback
```

**Web**
```bash
# Start dev server with debug output
npm run dev -- --debug

# Check for type errors
npm run type-check
```

## Performance Considerations

### Rust

- Use `Vec` with pre-allocated capacity when size is known
- Avoid unnecessary clones - use references where possible
- Use `#[inline]` for small, frequently-called functions

### Web

- Use `React.memo` for expensive components
- Virtualize long lists (react-window or react-virtualized)
- Lazy load card images
- Debounce user input (search, sliders)

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [React Documentation](https://react.dev/)
- [Vitest Documentation](https://vitest.dev/)
- [Testing Library](https://testing-library.com/)
