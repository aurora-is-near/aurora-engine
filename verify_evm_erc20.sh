rm etc/eth-contracts/res/*.bin etc/eth-contracts/res/*.hex
make etc/eth-contracts/res/EvmErc20.bin
if [[ $(git diff etc/eth-contracts/res/) ]]; then
	echo "Error EvmErc20.bin not up to date"
	exit 1
else
	exit 0
fi
