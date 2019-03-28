#! /bin/sh

docker run -it -v $(pwd)/examples:/setup --name rabbit --network=host rabbitmq:3.7-management

