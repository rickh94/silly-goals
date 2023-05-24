#!/usr/bin/env bash
litestream restore -if-db-not-exists -if-replica-exists -config /etc/litestream.yml /data/db/silly-docker-dev.db
if [ ! -f /data/db/silly-docker-dev.db ]; then
	touch /data/db/silly-docker-dev.db
fi 
litestream replicate -config /etc/litestream.yml 
