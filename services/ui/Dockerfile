# Ferrum UI: build React app and serve with nginx
FROM node:22-alpine AS builder
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

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
RUN echo 'server { listen 80; root /usr/share/nginx/html; index index.html; location / { try_files $uri $uri/ /index.html; } }' > /etc/nginx/conf.d/default.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
