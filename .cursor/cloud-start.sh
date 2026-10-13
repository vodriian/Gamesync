#!/usr/bin/env bash
# Start Postgres and the web reference app. Safe to run again.
# The native desktop app is started on demand; it needs a display.
set -euo pipefail

if ! pg_isready -q; then
  sudo pg_ctlcluster 16 main start
fi

ready=0
for _ in $(seq 1 30); do
  if pg_isready -q; then
    ready=1
    break
  fi
  sleep 1
done
if [[ "$ready" -ne 1 ]]; then
  echo "Postgres did not become ready" >&2
  exit 1
fi

sudo -u postgres psql -v ON_ERROR_STOP=1 -c "ALTER USER postgres WITH PASSWORD 'postgres';"
if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='gamesync'" | grep -q 1; then
  sudo -u postgres createdb gamesync
fi

export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/gamesync"
npx drizzle-kit push --force

# npm run dev binds localhost, which resolves to IPv6 on this image.
if curl -sf -o /dev/null --max-time 2 http://localhost:5001/api/columns; then
  exit 0
fi

exec npm run dev
