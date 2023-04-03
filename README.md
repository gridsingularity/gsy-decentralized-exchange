# GSy DEX

### Run Service using Docker Compose

First, install [Docker](https://docs.docker.com/get-docker/) and
[Docker Compose](https://docs.docker.com/compose/install/).

Build and tag the docker image:

```bash
docker build -t rust_build_image .
docker tag rust_build_image:latest rust_build_image:staging
```
and start docker-compose:

```bash
docker-compose up
```