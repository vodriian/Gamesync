# GameSync - Steam to Notion

## Overview

GameSync is a web application that helps users manage and sync their Steam game library to Notion databases. The app fetches games from a user's Steam account, enriches them with ProtonDB compatibility ratings (for Linux/Steam Deck gaming), and provides tools to sync this data to Notion or Craft documents. It features a dark "gamer" aesthetic with card and table views for browsing games.

## User Preferences

Preferred communication style: Simple, everyday language.

## System Architecture

### Frontend Architecture
- **Framework**: React 18 with TypeScript
- **Routing**: Wouter (lightweight React router)
- **State Management**: TanStack React Query for server state caching and synchronization
- **Styling**: Tailwind CSS v4 with custom dark theme optimized for gaming aesthetics
- **UI Components**: shadcn/ui component library built on Radix UI primitives
- **Animations**: Framer Motion for smooth transitions and drag-and-drop functionality
- **Build Tool**: Vite with custom plugins for Replit integration

### Backend Architecture
- **Runtime**: Node.js with Express.js
- **Language**: TypeScript with ES modules
- **API Pattern**: RESTful JSON API at `/api/*` routes
- **Database ORM**: Drizzle ORM with PostgreSQL dialect
- **Schema Validation**: Zod with drizzle-zod integration for type-safe validation

### Data Storage
- **Database**: PostgreSQL (configured via `DATABASE_URL` environment variable)
- **Tables**:
  - `user_config`: Stores API keys and integration settings (Steam, Notion, Craft)
  - `custom_columns`: User-defined column configurations for game properties
  - `games`: Cached game data with custom properties and sync status

### Project Structure
```
client/           # React frontend application
  src/
    components/   # Reusable UI components
    pages/        # Route-level page components
    lib/          # Utilities, API service, types
    hooks/        # Custom React hooks
server/           # Express backend
  services/       # External API integrations (Steam, ProtonDB, Notion, Craft)
  routes.ts       # API route definitions
  storage.ts      # Database access layer
shared/           # Shared code between client and server
  schema.ts       # Drizzle database schema and Zod validators
```

### Key Design Decisions

1. **Monorepo Structure**: Client and server share TypeScript types via the `shared/` directory, ensuring type safety across the full stack.

2. **Service Layer Pattern**: External API integrations (Steam, ProtonDB, Notion, Craft) are encapsulated in dedicated service classes under `server/services/`.

3. **Development vs Production**: In development, Vite serves the frontend with HMR. In production, the client is built and served as static files from the Express server.

4. **Custom Column System**: Users can define their own game properties with various types (text, number, select, multi-select, date, url) stored as JSONB in the database.

## External Dependencies

### Third-Party APIs
- **Steam Web API**: Fetches user's owned games library (requires `STEAM_KEY` and `STEAMID64`)
- **ProtonDB API**: Retrieves Linux/Steam Deck compatibility ratings for games
- **Notion API**: Creates and updates pages in Notion databases (requires `NOTION_TOKEN` and database ID)
- **Craft API**: Optional integration for syncing to Craft documents

### Required Environment Variables
- `DATABASE_URL`: PostgreSQL connection string
- `STEAM_KEY`: Steam Web API key (for game library access)
- `STEAMID64`: User's Steam 64-bit ID
- `NOTION_TOKEN`: Notion integration token
- `NOTION_DATABASE_ID`: Target Notion database ID

### Database Migrations
Run `npm run db:push` to push schema changes to the database using Drizzle Kit.