# Development Guide

This guide covers development practices, testing, and code style for the Flashback MTGO Replay System.

## Prerequisites

- .NET 10+ (for the recorder — MTGOSDK requires net10.0-windows)
- Node.js 18+
- npm

## Project Setup

```bash
# Clone repository
git clone <repository-url>
cd flashback

# Install web dependencies
cd web
npm install
cd ..
```

## Development Workflow

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

### TypeScript

- Use Prettier for formatting (configured in `.prettierrc`)
- Use ESLint for linting
- Follow TypeScript naming conventions:
  - Variables and functions: `camelCase`
  - Classes and interfaces: `PascalCase`
  - Constants: `UPPER_SNAKE_CASE`
- Use functional components with hooks
- Document complex components with JSDoc comments

### C# (Recorder)

- Follow standard .NET naming conventions
- Use nullable reference types
- Async/await for all I/O operations

## Commit Convention

Follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` - New feature
- `fix:` - Bug fix
- `test:` - Adding or updating tests
- `docs:` - Documentation changes
- `refactor:` - Code refactoring
- `chore:` - Maintenance tasks

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

**Web viewer won't load replays**
- Check browser console for errors
- Verify replay file format matches v3 schema
- Check API endpoint configuration

### Debugging Tips

**Web**
```bash
# Start dev server with debug output
npm run dev -- --debug

# Check for type errors
npm run type-check
```

## Performance Considerations

### Web

- Use `React.memo` for expensive components
- Virtualize long lists (react-window or react-virtualized)
- Lazy load card images
- Debounce user input (search, sliders)

## Resources

- [React Documentation](https://react.dev/)
- [Vitest Documentation](https://vitest.dev/)
- [Testing Library](https://testing-library.com/)
- [MTGOSDK](https://github.com/videre-project/MTGOSDK)
