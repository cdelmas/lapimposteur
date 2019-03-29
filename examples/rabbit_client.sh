#!/bin/sh

docker exec -it rabbit sh -c '
  rabbitmqadmin declare queue name=client durable=false -V test -u admin -p king &&
  rabbitmqadmin declare binding source=bob-x destination_type=queue destination=client routing_key=r.k.2 -V test -u admin -p king'
