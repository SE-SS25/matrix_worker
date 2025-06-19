set dotenv-filename := "just.env"

default:
    @just --list

up:
    docker compose up --build --detach

down:
    docker compose down --remove-orphans

scale_count:
    @fish -c 'docker ps --format {{{{ .Names }} | rg "\-matrix" | wc -l'

scale count:
    docker compose up -d --scale matrix={{ count }}

rebuild: cache up

db_only:
    docker compose up --detach postgres mongo mongo-express

db_init: db_only
    @sleep 1
    sqlx database create
    sqlx migrate run

storage_rm:
    docker compose down --remove-orphans --volumes

storage_reset: storage_rm up

cache:
    cargo sqlx prepare --workspace

build: cache
    docker build ./

publish tag:
    docker logout
    echo $CR_PAT | docker login ghcr.io -u $CR_USERNAME --password-stdin

    docker build ./ -f docker/lord/release/Dockerfile -t "ghcr.io/se-ss25/matrix_worker:{{ tag }}"
    docker build ./ -f docker/lord/release/Dockerfile -t "ghcr.io/se-ss25/matrix_worker:latest"

    docker push "ghcr.io/se-ss25/matrix_worker:{{ tag }}"
    docker push "ghcr.io/se-ss25/matrix_worker:latest"

    docker logout
