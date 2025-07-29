#!/bin/bash
# Run tests in Linux Docker container
docker run --rm -v "$(pwd):/project" -w /project rust:latest cargo test