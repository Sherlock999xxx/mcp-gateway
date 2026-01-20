# syntax=docker/dockerfile:1

FROM node:20-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM node:20-alpine AS builder
WORKDIR /app
ARG UI_VERSION=dev
ENV NEXT_TELEMETRY_DISABLED=1
ENV NEXT_PUBLIC_UI_VERSION=$UI_VERSION
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN npm run build

FROM node:20-alpine AS runner
WORKDIR /app
ARG UI_VERSION=dev
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV NEXT_PUBLIC_UI_VERSION=$UI_VERSION
ENV PORT=3000
ENV HOSTNAME=0.0.0.0

RUN npm install -g npm@11.7.0 && npm cache clean --force

# Next.js "standalone" output (see next.config.ts)
COPY --from=builder /app/public ./public
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static

EXPOSE 3000
CMD ["node", "server.js"]
