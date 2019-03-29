#!/bin/sh


rabbitmqadmin declare queue name=client durable=false -V test -u admin -p king
rabbitmqadmin declare binding source=bob-x destination_type=queue destination=client routing_key=r.k.2 -V test -u admin -p king

rabbitmqadmin get queue=client -V test -u admin -p king
rabbitmqadmin publish exchange=bob-x routing_key=r.k.1 payload="hello, world"  -V test -u admin -p king