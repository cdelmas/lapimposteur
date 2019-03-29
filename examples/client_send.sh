#!/bin/sh

docker exec -it rabbit sh -c 'rabbitmqadmin publish exchange=bob-x routing_key=r.k.1 payload="hello, world"  -V test -u admin -p king'