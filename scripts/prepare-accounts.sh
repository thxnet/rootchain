#!/usr/bin/env bash
set -e


if [[ $# = 1 && $1 = *[[:digit:]]* ]];
then
	SEEDS=$(seq 1 $@)
else
	SEEDS=$@
fi

generate_account_id() {
	subkey inspect "$SECRET//$1" | grep "Account ID" | awk '{ print $3 }'
}

generate_address() {
	subkey inspect "$SECRET//$1" | grep "SS58 Address" | awk '{ print $3 }'
}

generate_address_and_account_id() {
	ACCOUNT=$(generate_account_id $1)
	ADDRESS=$(generate_address $1)
	if ${4:-false}; then
		INTO="unchecked_into"
	else
		INTO="into"
	fi

	printf "// $ADDRESS\nhex![\"${ACCOUNT#'0x'}\"].$INTO(),"
}

AUTHORITIES=""

for i in $SEEDS; do
	AUTHORITIES+="// $i\n"
	AUTHORITIES+="$(generate_address_and_account_id $i)\n"
done

printf "$AUTHORITIES"

