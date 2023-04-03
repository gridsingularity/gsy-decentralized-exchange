## Run Service using Docker Compose

First, install <a href="https://docs.docker.com/get-docker/" target="_blank">Docker</a> and <a href="https://docs.docker.com/compose/install/" target="_blank">Docker Compose</a>.

Build and tag the docker image:

```bash
docker build -t rust_build_image .
docker tag rust_build_image:latest rust_build_image:staging
```
and start docker-compose:

```bash
docker-compose up
```