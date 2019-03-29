#!/bin/sh

docker exec -it rabbit sh -c 'rabbitmqadmin get queue=client count=10 ackmode=ack_requeue_false -V test -u admin -p king'