## Run Service using Docker Compose

First, install <a href="https://docs.docker.com/get-docker/" target="_blank">Docker</a> and <a href="https://docs.docker.com/compose/install/" target="_blank">Docker Compose</a>.

Build and tag the docker image:

```bash
docker build -t gsy_dex_image .
docker tag gsy_dex_image:latest gsy_dex_image:staging
```
and start docker-compose:

```bash
docker-compose up
```