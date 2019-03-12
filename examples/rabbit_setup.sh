#/bin/sh

rabbitmqctl add_user admin king
rabbitmqctl set_user_tags admin administrator
rabbitmqctl set_permissions -p / admin ".*" ".*" ".*"
rabbitmqctl add_vhost test
rabbitmqctl set_permissions -p test admin ".*" ".*" ".*"
rabbitmqctl add_user bob bob
rabbitmqctl set_permissions -p test bob "^bob-.*" "^bob-.*" "^bob-.*"
rabbitmqadmin declare exchange -V test name=bob-x type=direct -u admin -p king