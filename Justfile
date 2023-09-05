start:
	#!/usr/bin/env bash
	if [ ! -d '.pids' ]; then
		mkdir '.pids'
	fi
	caddy run --config $CADDYFILE &
	echo $! > .pids/caddy
	redis-server --port $REDIS_PORT &
	echo $! > .pids/redis

stop:
	#!/usr/bin/env bash
	if [ -d '.pids' ]; then
		for f in `ls .pids`; do
			kill $(cat .pids/$f)
			rm .pids/$f
		done
	fi
