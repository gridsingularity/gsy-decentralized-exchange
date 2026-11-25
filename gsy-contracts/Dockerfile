FROM node:22-alpine

WORKDIR /app

RUN apk add --no-cache git

COPY package.json package-lock.json* ./

RUN npm install

COPY . .

CMD ["npx", "hardhat", "test"]
