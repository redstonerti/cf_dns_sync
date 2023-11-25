# cf_dns_sync
A little ip changer using cloudflare's api. Useful for people with non - static ip addresses

This program is meant to be run indefinitely with no user input after the inital configuration (unless you change your dns records).

It will use your cloudflare credentials and the cloudflare api to periodically set the content of the type A records that you specify to your public ip address.

After installing, run `cf_dns_sync configure`

After configuring, simply run `cf_dns_sync` and forget about it.

