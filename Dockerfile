# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS web-dependencies

WORKDIR /app

COPY web/package.json web/package-lock.json ./
RUN npm ci

FROM web-dependencies AS web-builder

COPY web ./
# Next.js expects this directory in the standalone image even when the
# application does not currently ship public assets.
RUN mkdir -p public
RUN npm run build

FROM node:22-bookworm-slim AS web-runtime

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV HOSTNAME=0.0.0.0
ENV PORT=3000

WORKDIR /app

RUN groupadd --system --gid 1001 nodejs \
  && useradd --system --uid 1001 --gid nodejs nextjs

COPY --from=web-builder --chown=nextjs:nodejs /app/public ./public
COPY --from=web-builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=web-builder --chown=nextjs:nodejs /app/.next/static ./.next/static

USER nextjs

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:' + process.env.PORT + '/').then((response) => process.exit(response.ok ? 0 : 1)).catch(() => process.exit(1))"

CMD ["node", "server.js"]
