# Ferrum UI: build React app and serve with nginx
# Tag: node:22-alpine
FROM node:22-alpine@sha256:0a7108bf6c7bf5de370ffb1a3ed6be93d405b43ff159f681a8d18c0e2bc2e402 AS builder
WORKDIR /app
ARG VITE_ENABLE_PROBLEM_REPORT=true
ARG VITE_PROBLEM_REPORT_EMAIL=contact@synapticfour.com
ARG VITE_PROBLEM_REPORT_GITHUB_REPO=SynapticFour/Ferrum
ENV VITE_ENABLE_PROBLEM_REPORT=$VITE_ENABLE_PROBLEM_REPORT
ENV VITE_PROBLEM_REPORT_EMAIL=$VITE_PROBLEM_REPORT_EMAIL
ENV VITE_PROBLEM_REPORT_GITHUB_REPO=$VITE_PROBLEM_REPORT_GITHUB_REPO
COPY package.json package-lock.json* ./
RUN npm ci 2>/dev/null || npm install
COPY . .
RUN npm run build

# Tag: nginx:alpine
FROM nginx:alpine@sha256:df221db836e1754089190208cee7eeda94f233197056426eda74a43ab1abeac2
COPY --from=builder /app/dist /usr/share/nginx/html
RUN echo 'server { listen 80; root /usr/share/nginx/html; index index.html; location / { try_files $uri $uri/ /index.html; } }' > /etc/nginx/conf.d/default.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
