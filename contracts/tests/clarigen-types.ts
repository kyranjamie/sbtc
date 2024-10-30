import type {
  TypedAbiArg,
  TypedAbiFunction,
  TypedAbiMap,
  TypedAbiVariable,
  Response,
} from "@clarigen/core";

export const contracts = {
  sbtcBootstrapSigners: {
    functions: {
      signerKeyLengthCheck: {
        name: "signer-key-length-check",
        access: "private",
        args: [
          { name: "current-key", type: { buffer: { length: 33 } } },
          {
            name: "helper-response",
            type: { response: { ok: "uint128", error: "uint128" } },
          },
        ],
        outputs: { type: { response: { ok: "uint128", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          currentKey: TypedAbiArg<Uint8Array, "currentKey">,
          helperResponse: TypedAbiArg<
            Response<number | bigint, number | bigint>,
            "helperResponse"
          >,
        ],
        Response<bigint, bigint>
      >,
      rotateKeysWrapper: {
        name: "rotate-keys-wrapper",
        access: "public",
        args: [
          {
            name: "new-keys",
            type: { list: { type: { buffer: { length: 33 } }, length: 128 } },
          },
          { name: "new-aggregate-pubkey", type: { buffer: { length: 33 } } },
          { name: "new-signature-threshold", type: "uint128" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          newKeys: TypedAbiArg<Uint8Array[], "newKeys">,
          newAggregatePubkey: TypedAbiArg<Uint8Array, "newAggregatePubkey">,
          newSignatureThreshold: TypedAbiArg<
            number | bigint,
            "newSignatureThreshold"
          >,
        ],
        Response<boolean, bigint>
      >,
      bytesLen: {
        name: "bytes-len",
        access: "read_only",
        args: [{ name: "bytes", type: { buffer: { length: 33 } } }],
        outputs: { type: { buffer: { length: 1 } } },
      } as TypedAbiFunction<
        [bytes: TypedAbiArg<Uint8Array, "bytes">],
        Uint8Array
      >,
      concatPubkeysFold: {
        name: "concat-pubkeys-fold",
        access: "read_only",
        args: [
          { name: "pubkey", type: { buffer: { length: 33 } } },
          { name: "iterator", type: { buffer: { length: 510 } } },
        ],
        outputs: { type: { buffer: { length: 510 } } },
      } as TypedAbiFunction<
        [
          pubkey: TypedAbiArg<Uint8Array, "pubkey">,
          iterator: TypedAbiArg<Uint8Array, "iterator">,
        ],
        Uint8Array
      >,
      pubkeysToBytes: {
        name: "pubkeys-to-bytes",
        access: "read_only",
        args: [
          {
            name: "pubkeys",
            type: { list: { type: { buffer: { length: 33 } }, length: 128 } },
          },
        ],
        outputs: { type: { buffer: { length: 510 } } },
      } as TypedAbiFunction<
        [pubkeys: TypedAbiArg<Uint8Array[], "pubkeys">],
        Uint8Array
      >,
      pubkeysToHash: {
        name: "pubkeys-to-hash",
        access: "read_only",
        args: [
          {
            name: "pubkeys",
            type: { list: { type: { buffer: { length: 33 } }, length: 128 } },
          },
          { name: "m", type: "uint128" },
        ],
        outputs: { type: { buffer: { length: 20 } } },
      } as TypedAbiFunction<
        [
          pubkeys: TypedAbiArg<Uint8Array[], "pubkeys">,
          m: TypedAbiArg<number | bigint, "m">,
        ],
        Uint8Array
      >,
      pubkeysToPrincipal: {
        name: "pubkeys-to-principal",
        access: "read_only",
        args: [
          {
            name: "pubkeys",
            type: { list: { type: { buffer: { length: 33 } }, length: 128 } },
          },
          { name: "m", type: "uint128" },
        ],
        outputs: { type: "principal" },
      } as TypedAbiFunction<
        [
          pubkeys: TypedAbiArg<Uint8Array[], "pubkeys">,
          m: TypedAbiArg<number | bigint, "m">,
        ],
        string
      >,
      pubkeysToSpendScript: {
        name: "pubkeys-to-spend-script",
        access: "read_only",
        args: [
          {
            name: "pubkeys",
            type: { list: { type: { buffer: { length: 33 } }, length: 128 } },
          },
          { name: "m", type: "uint128" },
        ],
        outputs: { type: { buffer: { length: 513 } } },
      } as TypedAbiFunction<
        [
          pubkeys: TypedAbiArg<Uint8Array[], "pubkeys">,
          m: TypedAbiArg<number | bigint, "m">,
        ],
        Uint8Array
      >,
      uintToByte: {
        name: "uint-to-byte",
        access: "read_only",
        args: [{ name: "n", type: "uint128" }],
        outputs: { type: { buffer: { length: 1 } } },
      } as TypedAbiFunction<[n: TypedAbiArg<number | bigint, "n">], Uint8Array>,
    },
    maps: {},
    variables: {
      BUFF_TO_BYTE: {
        name: "BUFF_TO_BYTE",
        type: {
          list: {
            type: {
              buffer: {
                length: 1,
              },
            },
            length: 256,
          },
        },
        access: "constant",
      } as TypedAbiVariable<Uint8Array[]>,
      ERR_INVALID_CALLER: {
        name: "ERR_INVALID_CALLER",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_KEY_SIZE: {
        name: "ERR_KEY_SIZE",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_KEY_SIZE_PREFIX: {
        name: "ERR_KEY_SIZE_PREFIX",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
      ERR_SIGNATURE_THRESHOLD: {
        name: "ERR_SIGNATURE_THRESHOLD",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      keySize: {
        name: "key-size",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
    },
    constants: {},
    non_fungible_tokens: [],
    fungible_tokens: [],
    epoch: "Epoch30",
    clarity_version: "Clarity3",
    contractName: "sbtc-bootstrap-signers",
  },
  sbtcDeposit: {
    functions: {
      completeIndividualDepositsHelper: {
        name: "complete-individual-deposits-helper",
        access: "private",
        args: [
          {
            name: "deposit",
            type: {
              tuple: [
                { name: "amount", type: "uint128" },
                { name: "burn-hash", type: { buffer: { length: 32 } } },
                { name: "burn-height", type: "uint128" },
                { name: "recipient", type: "principal" },
                { name: "txid", type: { buffer: { length: 32 } } },
                { name: "vout-index", type: "uint128" },
              ],
            },
          },
          {
            name: "helper-response",
            type: { response: { ok: "uint128", error: "uint128" } },
          },
        ],
        outputs: { type: { response: { ok: "uint128", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          deposit: TypedAbiArg<
            {
              amount: number | bigint;
              burnHash: Uint8Array;
              burnHeight: number | bigint;
              recipient: string;
              txid: Uint8Array;
              voutIndex: number | bigint;
            },
            "deposit"
          >,
          helperResponse: TypedAbiArg<
            Response<number | bigint, number | bigint>,
            "helperResponse"
          >,
        ],
        Response<bigint, bigint>
      >,
      completeDepositWrapper: {
        name: "complete-deposit-wrapper",
        access: "public",
        args: [
          { name: "txid", type: { buffer: { length: 32 } } },
          { name: "vout-index", type: "uint128" },
          { name: "amount", type: "uint128" },
          { name: "recipient", type: "principal" },
          { name: "burn-hash", type: { buffer: { length: 32 } } },
          { name: "burn-height", type: "uint128" },
        ],
        outputs: {
          type: {
            response: {
              ok: { response: { ok: "bool", error: "uint128" } },
              error: "uint128",
            },
          },
        },
      } as TypedAbiFunction<
        [
          txid: TypedAbiArg<Uint8Array, "txid">,
          voutIndex: TypedAbiArg<number | bigint, "voutIndex">,
          amount: TypedAbiArg<number | bigint, "amount">,
          recipient: TypedAbiArg<string, "recipient">,
          burnHash: TypedAbiArg<Uint8Array, "burnHash">,
          burnHeight: TypedAbiArg<number | bigint, "burnHeight">,
        ],
        Response<Response<boolean, bigint>, bigint>
      >,
      completeDepositsWrapper: {
        name: "complete-deposits-wrapper",
        access: "public",
        args: [
          {
            name: "deposits",
            type: {
              list: {
                type: {
                  tuple: [
                    { name: "amount", type: "uint128" },
                    { name: "burn-hash", type: { buffer: { length: 32 } } },
                    { name: "burn-height", type: "uint128" },
                    { name: "recipient", type: "principal" },
                    { name: "txid", type: { buffer: { length: 32 } } },
                    { name: "vout-index", type: "uint128" },
                  ],
                },
                length: 650,
              },
            },
          },
        ],
        outputs: { type: { response: { ok: "uint128", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          deposits: TypedAbiArg<
            {
              amount: number | bigint;
              burnHash: Uint8Array;
              burnHeight: number | bigint;
              recipient: string;
              txid: Uint8Array;
              voutIndex: number | bigint;
            }[],
            "deposits"
          >,
        ],
        Response<bigint, bigint>
      >,
      getBurnHeader: {
        name: "get-burn-header",
        access: "read_only",
        args: [{ name: "height", type: "uint128" }],
        outputs: { type: { optional: { buffer: { length: 32 } } } },
      } as TypedAbiFunction<
        [height: TypedAbiArg<number | bigint, "height">],
        Uint8Array | null
      >,
    },
    maps: {},
    variables: {
      ERR_DEPOSIT: {
        name: "ERR_DEPOSIT",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_DEPOSIT_INDEX_PREFIX: {
        name: "ERR_DEPOSIT_INDEX_PREFIX",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
      ERR_DEPOSIT_REPLAY: {
        name: "ERR_DEPOSIT_REPLAY",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_INVALID_BURN_HASH: {
        name: "ERR_INVALID_BURN_HASH",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_INVALID_CALLER: {
        name: "ERR_INVALID_CALLER",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_LOWER_THAN_DUST: {
        name: "ERR_LOWER_THAN_DUST",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_TXID_LEN: {
        name: "ERR_TXID_LEN",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      dustLimit: {
        name: "dust-limit",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
      txidLength: {
        name: "txid-length",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
    },
    constants: {
      ERR_DEPOSIT: {
        isOk: false,
        value: 303n,
      },
      ERR_DEPOSIT_INDEX_PREFIX: 303n,
      ERR_DEPOSIT_REPLAY: {
        isOk: false,
        value: 301n,
      },
      ERR_INVALID_BURN_HASH: {
        isOk: false,
        value: 305n,
      },
      ERR_INVALID_CALLER: {
        isOk: false,
        value: 304n,
      },
      ERR_LOWER_THAN_DUST: {
        isOk: false,
        value: 302n,
      },
      ERR_TXID_LEN: {
        isOk: false,
        value: 300n,
      },
      dustLimit: 546n,
      txidLength: 32n,
    },
    non_fungible_tokens: [],
    fungible_tokens: [],
    epoch: "Epoch30",
    clarity_version: "Clarity3",
    contractName: "sbtc-deposit",
  },
  sbtcRegistry: {
    functions: {
      incrementLastWithdrawalRequestId: {
        name: "increment-last-withdrawal-request-id",
        access: "private",
        args: [],
        outputs: { type: "uint128" },
      } as TypedAbiFunction<[], bigint>,
      completeDeposit: {
        name: "complete-deposit",
        access: "public",
        args: [
          { name: "txid", type: { buffer: { length: 32 } } },
          { name: "vout-index", type: "uint128" },
          { name: "amount", type: "uint128" },
          { name: "recipient", type: "principal" },
          { name: "burn-hash", type: { buffer: { length: 32 } } },
          { name: "burn-height", type: "uint128" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          txid: TypedAbiArg<Uint8Array, "txid">,
          voutIndex: TypedAbiArg<number | bigint, "voutIndex">,
          amount: TypedAbiArg<number | bigint, "amount">,
          recipient: TypedAbiArg<string, "recipient">,
          burnHash: TypedAbiArg<Uint8Array, "burnHash">,
          burnHeight: TypedAbiArg<number | bigint, "burnHeight">,
        ],
        Response<boolean, bigint>
      >,
      completeWithdrawalAccept: {
        name: "complete-withdrawal-accept",
        access: "public",
        args: [
          { name: "request-id", type: "uint128" },
          { name: "bitcoin-txid", type: { buffer: { length: 32 } } },
          { name: "output-index", type: "uint128" },
          { name: "signer-bitmap", type: "uint128" },
          { name: "fee", type: "uint128" },
          { name: "burn-hash", type: { buffer: { length: 32 } } },
          { name: "burn-height", type: "uint128" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          requestId: TypedAbiArg<number | bigint, "requestId">,
          bitcoinTxid: TypedAbiArg<Uint8Array, "bitcoinTxid">,
          outputIndex: TypedAbiArg<number | bigint, "outputIndex">,
          signerBitmap: TypedAbiArg<number | bigint, "signerBitmap">,
          fee: TypedAbiArg<number | bigint, "fee">,
          burnHash: TypedAbiArg<Uint8Array, "burnHash">,
          burnHeight: TypedAbiArg<number | bigint, "burnHeight">,
        ],
        Response<boolean, bigint>
      >,
      completeWithdrawalReject: {
        name: "complete-withdrawal-reject",
        access: "public",
        args: [
          { name: "request-id", type: "uint128" },
          { name: "signer-bitmap", type: "uint128" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          requestId: TypedAbiArg<number | bigint, "requestId">,
          signerBitmap: TypedAbiArg<number | bigint, "signerBitmap">,
        ],
        Response<boolean, bigint>
      >,
      createWithdrawalRequest: {
        name: "create-withdrawal-request",
        access: "public",
        args: [
          { name: "amount", type: "uint128" },
          { name: "max-fee", type: "uint128" },
          { name: "sender", type: "principal" },
          {
            name: "recipient",
            type: {
              tuple: [
                { name: "hashbytes", type: { buffer: { length: 32 } } },
                { name: "version", type: { buffer: { length: 1 } } },
              ],
            },
          },
          { name: "height", type: "uint128" },
        ],
        outputs: { type: { response: { ok: "uint128", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          amount: TypedAbiArg<number | bigint, "amount">,
          maxFee: TypedAbiArg<number | bigint, "maxFee">,
          sender: TypedAbiArg<string, "sender">,
          recipient: TypedAbiArg<
            {
              hashbytes: Uint8Array;
              version: Uint8Array;
            },
            "recipient"
          >,
          height: TypedAbiArg<number | bigint, "height">,
        ],
        Response<bigint, bigint>
      >,
      rotateKeys: {
        name: "rotate-keys",
        access: "public",
        args: [
          {
            name: "new-keys",
            type: { list: { type: { buffer: { length: 33 } }, length: 128 } },
          },
          { name: "new-address", type: "principal" },
          { name: "new-aggregate-pubkey", type: { buffer: { length: 33 } } },
          { name: "new-signature-threshold", type: "uint128" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          newKeys: TypedAbiArg<Uint8Array[], "newKeys">,
          newAddress: TypedAbiArg<string, "newAddress">,
          newAggregatePubkey: TypedAbiArg<Uint8Array, "newAggregatePubkey">,
          newSignatureThreshold: TypedAbiArg<
            number | bigint,
            "newSignatureThreshold"
          >,
        ],
        Response<boolean, bigint>
      >,
      getCompletedDeposit: {
        name: "get-completed-deposit",
        access: "read_only",
        args: [
          { name: "txid", type: { buffer: { length: 32 } } },
          { name: "vout-index", type: "uint128" },
        ],
        outputs: {
          type: {
            optional: {
              tuple: [
                { name: "amount", type: "uint128" },
                { name: "recipient", type: "principal" },
              ],
            },
          },
        },
      } as TypedAbiFunction<
        [
          txid: TypedAbiArg<Uint8Array, "txid">,
          voutIndex: TypedAbiArg<number | bigint, "voutIndex">,
        ],
        {
          amount: bigint;
          recipient: string;
        } | null
      >,
      getCurrentAggregatePubkey: {
        name: "get-current-aggregate-pubkey",
        access: "read_only",
        args: [],
        outputs: { type: { buffer: { length: 33 } } },
      } as TypedAbiFunction<[], Uint8Array>,
      getCurrentSignerData: {
        name: "get-current-signer-data",
        access: "read_only",
        args: [],
        outputs: {
          type: {
            tuple: [
              {
                name: "current-aggregate-pubkey",
                type: { buffer: { length: 33 } },
              },
              { name: "current-signature-threshold", type: "uint128" },
              { name: "current-signer-principal", type: "principal" },
              {
                name: "current-signer-set",
                type: {
                  list: { type: { buffer: { length: 33 } }, length: 128 },
                },
              },
            ],
          },
        },
      } as TypedAbiFunction<
        [],
        {
          currentAggregatePubkey: Uint8Array;
          currentSignatureThreshold: bigint;
          currentSignerPrincipal: string;
          currentSignerSet: Uint8Array[];
        }
      >,
      getCurrentSignerPrincipal: {
        name: "get-current-signer-principal",
        access: "read_only",
        args: [],
        outputs: { type: "principal" },
      } as TypedAbiFunction<[], string>,
      getCurrentSignerSet: {
        name: "get-current-signer-set",
        access: "read_only",
        args: [],
        outputs: {
          type: { list: { type: { buffer: { length: 33 } }, length: 128 } },
        },
      } as TypedAbiFunction<[], Uint8Array[]>,
      getWithdrawalRequest: {
        name: "get-withdrawal-request",
        access: "read_only",
        args: [{ name: "id", type: "uint128" }],
        outputs: {
          type: {
            optional: {
              tuple: [
                { name: "amount", type: "uint128" },
                { name: "block-height", type: "uint128" },
                { name: "max-fee", type: "uint128" },
                {
                  name: "recipient",
                  type: {
                    tuple: [
                      { name: "hashbytes", type: { buffer: { length: 32 } } },
                      { name: "version", type: { buffer: { length: 1 } } },
                    ],
                  },
                },
                { name: "sender", type: "principal" },
                { name: "status", type: { optional: "bool" } },
              ],
            },
          },
        },
      } as TypedAbiFunction<
        [id: TypedAbiArg<number | bigint, "id">],
        {
          amount: bigint;
          blockHeight: bigint;
          maxFee: bigint;
          recipient: {
            hashbytes: Uint8Array;
            version: Uint8Array;
          };
          sender: string;
          status: boolean | null;
        } | null
      >,
      isProtocolCaller: {
        name: "is-protocol-caller",
        access: "read_only",
        args: [],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<[], Response<boolean, bigint>>,
      validateProtocolCaller: {
        name: "validate-protocol-caller",
        access: "read_only",
        args: [{ name: "caller", type: "principal" }],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [caller: TypedAbiArg<string, "caller">],
        Response<boolean, bigint>
      >,
    },
    maps: {
      aggregatePubkeys: {
        name: "aggregate-pubkeys",
        key: { buffer: { length: 33 } },
        value: "bool",
      } as TypedAbiMap<Uint8Array, boolean>,
      completedDeposits: {
        name: "completed-deposits",
        key: {
          tuple: [
            { name: "txid", type: { buffer: { length: 32 } } },
            { name: "vout-index", type: "uint128" },
          ],
        },
        value: {
          tuple: [
            { name: "amount", type: "uint128" },
            { name: "recipient", type: "principal" },
          ],
        },
      } as TypedAbiMap<
        {
          txid: Uint8Array;
          voutIndex: number | bigint;
        },
        {
          amount: bigint;
          recipient: string;
        }
      >,
      multiSigAddress: {
        name: "multi-sig-address",
        key: "principal",
        value: "bool",
      } as TypedAbiMap<string, boolean>,
      protocolContracts: {
        name: "protocol-contracts",
        key: "principal",
        value: "bool",
      } as TypedAbiMap<string, boolean>,
      withdrawalRequests: {
        name: "withdrawal-requests",
        key: "uint128",
        value: {
          tuple: [
            { name: "amount", type: "uint128" },
            { name: "block-height", type: "uint128" },
            { name: "max-fee", type: "uint128" },
            {
              name: "recipient",
              type: {
                tuple: [
                  { name: "hashbytes", type: { buffer: { length: 32 } } },
                  { name: "version", type: { buffer: { length: 1 } } },
                ],
              },
            },
            { name: "sender", type: "principal" },
          ],
        },
      } as TypedAbiMap<
        number | bigint,
        {
          amount: bigint;
          blockHeight: bigint;
          maxFee: bigint;
          recipient: {
            hashbytes: Uint8Array;
            version: Uint8Array;
          };
          sender: string;
        }
      >,
      withdrawalStatus: {
        name: "withdrawal-status",
        key: "uint128",
        value: "bool",
      } as TypedAbiMap<number | bigint, boolean>,
    },
    variables: {
      ERR_AGG_PUBKEY_REPLAY: {
        name: "ERR_AGG_PUBKEY_REPLAY",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_INVALID_REQUEST_ID: {
        name: "ERR_INVALID_REQUEST_ID",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_MULTI_SIG_REPLAY: {
        name: "ERR_MULTI_SIG_REPLAY",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_UNAUTHORIZED: {
        name: "ERR_UNAUTHORIZED",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      currentAggregatePubkey: {
        name: "current-aggregate-pubkey",
        type: {
          buffer: {
            length: 33,
          },
        },
        access: "variable",
      } as TypedAbiVariable<Uint8Array>,
      currentSignatureThreshold: {
        name: "current-signature-threshold",
        type: "uint128",
        access: "variable",
      } as TypedAbiVariable<bigint>,
      currentSignerPrincipal: {
        name: "current-signer-principal",
        type: "principal",
        access: "variable",
      } as TypedAbiVariable<string>,
      currentSignerSet: {
        name: "current-signer-set",
        type: {
          list: {
            type: {
              buffer: {
                length: 33,
              },
            },
            length: 128,
          },
        },
        access: "variable",
      } as TypedAbiVariable<Uint8Array[]>,
      lastWithdrawalRequestId: {
        name: "last-withdrawal-request-id",
        type: "uint128",
        access: "variable",
      } as TypedAbiVariable<bigint>,
    },
    constants: {},
    non_fungible_tokens: [],
    fungible_tokens: [],
    epoch: "Epoch30",
    clarity_version: "Clarity3",
    contractName: "sbtc-registry",
  },
  sbtcToken: {
    functions: {
      protocolMintManyIter: {
        name: "protocol-mint-many-iter",
        access: "private",
        args: [
          {
            name: "item",
            type: {
              tuple: [
                { name: "amount", type: "uint128" },
                { name: "recipient", type: "principal" },
              ],
            },
          },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          item: TypedAbiArg<
            {
              amount: number | bigint;
              recipient: string;
            },
            "item"
          >,
        ],
        Response<boolean, bigint>
      >,
      protocolBurn: {
        name: "protocol-burn",
        access: "public",
        args: [
          { name: "amount", type: "uint128" },
          { name: "owner", type: "principal" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          amount: TypedAbiArg<number | bigint, "amount">,
          owner: TypedAbiArg<string, "owner">,
        ],
        Response<boolean, bigint>
      >,
      protocolBurnLocked: {
        name: "protocol-burn-locked",
        access: "public",
        args: [
          { name: "amount", type: "uint128" },
          { name: "owner", type: "principal" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          amount: TypedAbiArg<number | bigint, "amount">,
          owner: TypedAbiArg<string, "owner">,
        ],
        Response<boolean, bigint>
      >,
      protocolLock: {
        name: "protocol-lock",
        access: "public",
        args: [
          { name: "amount", type: "uint128" },
          { name: "owner", type: "principal" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          amount: TypedAbiArg<number | bigint, "amount">,
          owner: TypedAbiArg<string, "owner">,
        ],
        Response<boolean, bigint>
      >,
      protocolMint: {
        name: "protocol-mint",
        access: "public",
        args: [
          { name: "amount", type: "uint128" },
          { name: "recipient", type: "principal" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          amount: TypedAbiArg<number | bigint, "amount">,
          recipient: TypedAbiArg<string, "recipient">,
        ],
        Response<boolean, bigint>
      >,
      protocolMintMany: {
        name: "protocol-mint-many",
        access: "public",
        args: [
          {
            name: "recipients",
            type: {
              list: {
                type: {
                  tuple: [
                    { name: "amount", type: "uint128" },
                    { name: "recipient", type: "principal" },
                  ],
                },
                length: 200,
              },
            },
          },
        ],
        outputs: {
          type: {
            response: {
              ok: {
                list: {
                  type: { response: { ok: "bool", error: "uint128" } },
                  length: 200,
                },
              },
              error: "uint128",
            },
          },
        },
      } as TypedAbiFunction<
        [
          recipients: TypedAbiArg<
            {
              amount: number | bigint;
              recipient: string;
            }[],
            "recipients"
          >,
        ],
        Response<Response<boolean, bigint>[], bigint>
      >,
      protocolSetName: {
        name: "protocol-set-name",
        access: "public",
        args: [{ name: "new-name", type: { "string-ascii": { length: 32 } } }],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [newName: TypedAbiArg<string, "newName">],
        Response<boolean, bigint>
      >,
      protocolSetSymbol: {
        name: "protocol-set-symbol",
        access: "public",
        args: [
          { name: "new-symbol", type: { "string-ascii": { length: 10 } } },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [newSymbol: TypedAbiArg<string, "newSymbol">],
        Response<boolean, bigint>
      >,
      protocolSetTokenUri: {
        name: "protocol-set-token-uri",
        access: "public",
        args: [
          {
            name: "new-uri",
            type: { optional: { "string-utf8": { length: 256 } } },
          },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [newUri: TypedAbiArg<string | null, "newUri">],
        Response<boolean, bigint>
      >,
      protocolTransfer: {
        name: "protocol-transfer",
        access: "public",
        args: [
          { name: "amount", type: "uint128" },
          { name: "sender", type: "principal" },
          { name: "recipient", type: "principal" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          amount: TypedAbiArg<number | bigint, "amount">,
          sender: TypedAbiArg<string, "sender">,
          recipient: TypedAbiArg<string, "recipient">,
        ],
        Response<boolean, bigint>
      >,
      protocolUnlock: {
        name: "protocol-unlock",
        access: "public",
        args: [
          { name: "amount", type: "uint128" },
          { name: "owner", type: "principal" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          amount: TypedAbiArg<number | bigint, "amount">,
          owner: TypedAbiArg<string, "owner">,
        ],
        Response<boolean, bigint>
      >,
      transfer: {
        name: "transfer",
        access: "public",
        args: [
          { name: "amount", type: "uint128" },
          { name: "sender", type: "principal" },
          { name: "recipient", type: "principal" },
          { name: "memo", type: { optional: { buffer: { length: 34 } } } },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          amount: TypedAbiArg<number | bigint, "amount">,
          sender: TypedAbiArg<string, "sender">,
          recipient: TypedAbiArg<string, "recipient">,
          memo: TypedAbiArg<Uint8Array | null, "memo">,
        ],
        Response<boolean, bigint>
      >,
      getBalance: {
        name: "get-balance",
        access: "read_only",
        args: [{ name: "who", type: "principal" }],
        outputs: { type: { response: { ok: "uint128", error: "none" } } },
      } as TypedAbiFunction<
        [who: TypedAbiArg<string, "who">],
        Response<bigint, null>
      >,
      getBalanceAvailable: {
        name: "get-balance-available",
        access: "read_only",
        args: [{ name: "who", type: "principal" }],
        outputs: { type: { response: { ok: "uint128", error: "none" } } },
      } as TypedAbiFunction<
        [who: TypedAbiArg<string, "who">],
        Response<bigint, null>
      >,
      getBalanceLocked: {
        name: "get-balance-locked",
        access: "read_only",
        args: [{ name: "who", type: "principal" }],
        outputs: { type: { response: { ok: "uint128", error: "none" } } },
      } as TypedAbiFunction<
        [who: TypedAbiArg<string, "who">],
        Response<bigint, null>
      >,
      getDecimals: {
        name: "get-decimals",
        access: "read_only",
        args: [],
        outputs: { type: { response: { ok: "uint128", error: "none" } } },
      } as TypedAbiFunction<[], Response<bigint, null>>,
      getName: {
        name: "get-name",
        access: "read_only",
        args: [],
        outputs: {
          type: {
            response: { ok: { "string-ascii": { length: 32 } }, error: "none" },
          },
        },
      } as TypedAbiFunction<[], Response<string, null>>,
      getSymbol: {
        name: "get-symbol",
        access: "read_only",
        args: [],
        outputs: {
          type: {
            response: { ok: { "string-ascii": { length: 10 } }, error: "none" },
          },
        },
      } as TypedAbiFunction<[], Response<string, null>>,
      getTokenUri: {
        name: "get-token-uri",
        access: "read_only",
        args: [],
        outputs: {
          type: {
            response: {
              ok: { optional: { "string-utf8": { length: 256 } } },
              error: "none",
            },
          },
        },
      } as TypedAbiFunction<[], Response<string | null, null>>,
      getTotalSupply: {
        name: "get-total-supply",
        access: "read_only",
        args: [],
        outputs: { type: { response: { ok: "uint128", error: "none" } } },
      } as TypedAbiFunction<[], Response<bigint, null>>,
    },
    maps: {},
    variables: {
      ERR_NOT_AUTH: {
        name: "ERR_NOT_AUTH",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_NOT_OWNER: {
        name: "ERR_NOT_OWNER",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      tokenDecimals: {
        name: "token-decimals",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
      tokenName: {
        name: "token-name",
        type: {
          "string-ascii": {
            length: 32,
          },
        },
        access: "variable",
      } as TypedAbiVariable<string>,
      tokenSymbol: {
        name: "token-symbol",
        type: {
          "string-ascii": {
            length: 10,
          },
        },
        access: "variable",
      } as TypedAbiVariable<string>,
      tokenUri: {
        name: "token-uri",
        type: {
          optional: {
            "string-utf8": {
              length: 256,
            },
          },
        },
        access: "variable",
      } as TypedAbiVariable<string | null>,
    },
    constants: {},
    non_fungible_tokens: [],
    fungible_tokens: [{ name: "sbtc-token" }, { name: "sbtc-token-locked" }],
    epoch: "Epoch30",
    clarity_version: "Clarity3",
    contractName: "sbtc-token",
  },
  sbtcWithdrawal: {
    functions: {
      completeIndividualWithdrawalHelper: {
        name: "complete-individual-withdrawal-helper",
        access: "private",
        args: [
          {
            name: "withdrawal",
            type: {
              tuple: [
                {
                  name: "bitcoin-txid",
                  type: { optional: { buffer: { length: 32 } } },
                },
                { name: "burn-hash", type: { buffer: { length: 32 } } },
                { name: "burn-height", type: "uint128" },
                { name: "fee", type: { optional: "uint128" } },
                { name: "output-index", type: { optional: "uint128" } },
                { name: "request-id", type: "uint128" },
                { name: "signer-bitmap", type: "uint128" },
                { name: "status", type: "bool" },
              ],
            },
          },
          {
            name: "helper-response",
            type: { response: { ok: "uint128", error: "uint128" } },
          },
        ],
        outputs: { type: { response: { ok: "uint128", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          withdrawal: TypedAbiArg<
            {
              bitcoinTxid: Uint8Array | null;
              burnHash: Uint8Array;
              burnHeight: number | bigint;
              fee: number | bigint | null;
              outputIndex: number | bigint | null;
              requestId: number | bigint;
              signerBitmap: number | bigint;
              status: boolean;
            },
            "withdrawal"
          >,
          helperResponse: TypedAbiArg<
            Response<number | bigint, number | bigint>,
            "helperResponse"
          >,
        ],
        Response<bigint, bigint>
      >,
      acceptWithdrawalRequest: {
        name: "accept-withdrawal-request",
        access: "public",
        args: [
          { name: "request-id", type: "uint128" },
          { name: "bitcoin-txid", type: { buffer: { length: 32 } } },
          { name: "signer-bitmap", type: "uint128" },
          { name: "output-index", type: "uint128" },
          { name: "fee", type: "uint128" },
          { name: "burn-hash", type: { buffer: { length: 32 } } },
          { name: "burn-height", type: "uint128" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          requestId: TypedAbiArg<number | bigint, "requestId">,
          bitcoinTxid: TypedAbiArg<Uint8Array, "bitcoinTxid">,
          signerBitmap: TypedAbiArg<number | bigint, "signerBitmap">,
          outputIndex: TypedAbiArg<number | bigint, "outputIndex">,
          fee: TypedAbiArg<number | bigint, "fee">,
          burnHash: TypedAbiArg<Uint8Array, "burnHash">,
          burnHeight: TypedAbiArg<number | bigint, "burnHeight">,
        ],
        Response<boolean, bigint>
      >,
      completeWithdrawals: {
        name: "complete-withdrawals",
        access: "public",
        args: [
          {
            name: "withdrawals",
            type: {
              list: {
                type: {
                  tuple: [
                    {
                      name: "bitcoin-txid",
                      type: { optional: { buffer: { length: 32 } } },
                    },
                    { name: "burn-hash", type: { buffer: { length: 32 } } },
                    { name: "burn-height", type: "uint128" },
                    { name: "fee", type: { optional: "uint128" } },
                    { name: "output-index", type: { optional: "uint128" } },
                    { name: "request-id", type: "uint128" },
                    { name: "signer-bitmap", type: "uint128" },
                    { name: "status", type: "bool" },
                  ],
                },
                length: 600,
              },
            },
          },
        ],
        outputs: { type: { response: { ok: "uint128", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          withdrawals: TypedAbiArg<
            {
              bitcoinTxid: Uint8Array | null;
              burnHash: Uint8Array;
              burnHeight: number | bigint;
              fee: number | bigint | null;
              outputIndex: number | bigint | null;
              requestId: number | bigint;
              signerBitmap: number | bigint;
              status: boolean;
            }[],
            "withdrawals"
          >,
        ],
        Response<bigint, bigint>
      >,
      initiateWithdrawalRequest: {
        name: "initiate-withdrawal-request",
        access: "public",
        args: [
          { name: "amount", type: "uint128" },
          {
            name: "recipient",
            type: {
              tuple: [
                { name: "hashbytes", type: { buffer: { length: 32 } } },
                { name: "version", type: { buffer: { length: 1 } } },
              ],
            },
          },
          { name: "max-fee", type: "uint128" },
        ],
        outputs: { type: { response: { ok: "uint128", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          amount: TypedAbiArg<number | bigint, "amount">,
          recipient: TypedAbiArg<
            {
              hashbytes: Uint8Array;
              version: Uint8Array;
            },
            "recipient"
          >,
          maxFee: TypedAbiArg<number | bigint, "maxFee">,
        ],
        Response<bigint, bigint>
      >,
      rejectWithdrawalRequest: {
        name: "reject-withdrawal-request",
        access: "public",
        args: [
          { name: "request-id", type: "uint128" },
          { name: "signer-bitmap", type: "uint128" },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          requestId: TypedAbiArg<number | bigint, "requestId">,
          signerBitmap: TypedAbiArg<number | bigint, "signerBitmap">,
        ],
        Response<boolean, bigint>
      >,
      getBurnHeader: {
        name: "get-burn-header",
        access: "read_only",
        args: [{ name: "height", type: "uint128" }],
        outputs: { type: { optional: { buffer: { length: 32 } } } },
      } as TypedAbiFunction<
        [height: TypedAbiArg<number | bigint, "height">],
        Uint8Array | null
      >,
      validateRecipient: {
        name: "validate-recipient",
        access: "read_only",
        args: [
          {
            name: "recipient",
            type: {
              tuple: [
                { name: "hashbytes", type: { buffer: { length: 32 } } },
                { name: "version", type: { buffer: { length: 1 } } },
              ],
            },
          },
        ],
        outputs: { type: { response: { ok: "bool", error: "uint128" } } },
      } as TypedAbiFunction<
        [
          recipient: TypedAbiArg<
            {
              hashbytes: Uint8Array;
              version: Uint8Array;
            },
            "recipient"
          >,
        ],
        Response<boolean, bigint>
      >,
    },
    maps: {},
    variables: {
      DUST_LIMIT: {
        name: "DUST_LIMIT",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
      ERR_ALREADY_PROCESSED: {
        name: "ERR_ALREADY_PROCESSED",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_DUST_LIMIT: {
        name: "ERR_DUST_LIMIT",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_FEE_TOO_HIGH: {
        name: "ERR_FEE_TOO_HIGH",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_INVALID_ADDR_HASHBYTES: {
        name: "ERR_INVALID_ADDR_HASHBYTES",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_INVALID_ADDR_VERSION: {
        name: "ERR_INVALID_ADDR_VERSION",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_INVALID_BURN_HASH: {
        name: "ERR_INVALID_BURN_HASH",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_INVALID_CALLER: {
        name: "ERR_INVALID_CALLER",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_INVALID_REQUEST: {
        name: "ERR_INVALID_REQUEST",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_WITHDRAWAL_INDEX: {
        name: "ERR_WITHDRAWAL_INDEX",
        type: {
          response: {
            ok: "none",
            error: "uint128",
          },
        },
        access: "constant",
      } as TypedAbiVariable<Response<null, bigint>>,
      ERR_WITHDRAWAL_INDEX_PREFIX: {
        name: "ERR_WITHDRAWAL_INDEX_PREFIX",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
      MAX_ADDRESS_VERSION: {
        name: "MAX_ADDRESS_VERSION",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
      mAX_ADDRESS_VERSION_BUFF_20: {
        name: "MAX_ADDRESS_VERSION_BUFF_20",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
      mAX_ADDRESS_VERSION_BUFF_32: {
        name: "MAX_ADDRESS_VERSION_BUFF_32",
        type: "uint128",
        access: "constant",
      } as TypedAbiVariable<bigint>,
    },
    constants: {
      DUST_LIMIT: 546n,
      ERR_ALREADY_PROCESSED: {
        isOk: false,
        value: 505n,
      },
      ERR_DUST_LIMIT: {
        isOk: false,
        value: 502n,
      },
      ERR_FEE_TOO_HIGH: {
        isOk: false,
        value: 506n,
      },
      ERR_INVALID_ADDR_HASHBYTES: {
        isOk: false,
        value: 501n,
      },
      ERR_INVALID_ADDR_VERSION: {
        isOk: false,
        value: 500n,
      },
      ERR_INVALID_BURN_HASH: {
        isOk: false,
        value: 508n,
      },
      ERR_INVALID_CALLER: {
        isOk: false,
        value: 504n,
      },
      ERR_INVALID_REQUEST: {
        isOk: false,
        value: 503n,
      },
      ERR_WITHDRAWAL_INDEX: {
        isOk: false,
        value: 507n,
      },
      ERR_WITHDRAWAL_INDEX_PREFIX: 507n,
      MAX_ADDRESS_VERSION: 6n,
      mAX_ADDRESS_VERSION_BUFF_20: 4n,
      mAX_ADDRESS_VERSION_BUFF_32: 6n,
    },
    non_fungible_tokens: [],
    fungible_tokens: [],
    epoch: "Epoch30",
    clarity_version: "Clarity3",
    contractName: "sbtc-withdrawal",
  },
} as const;

export const accounts = {
  deployer: {
    address: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039",
    balance: "100000000000000",
  },
  faucet: {
    address: "STNHKEPYEPJ8ET55ZZ0M5A34J0R3N5FM2CMMMAZ6",
    balance: "100000000000000",
  },
  wallet_1: {
    address: "ST1YEHRRYJ4GF9CYBFFN0ZVCXX1APSBEEQ5KEDN7M",
    balance: "100000000000000",
  },
  wallet_2: {
    address: "ST1WNJTS9JM1JYGK758B10DBAMBZ0K23ADP392SBV",
    balance: "100000000000000",
  },
  wallet_3: {
    address: "ST1MDWBDVDGAANEH9001HGXQA6XRNK7PX7A7X8M6R",
    balance: "100000000000000",
  },
} as const;

export const identifiers = {
  sbtcBootstrapSigners:
    "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-bootstrap-signers",
  sbtcDeposit: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-deposit",
  sbtcRegistry: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-registry",
  sbtcToken: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-token",
  sbtcWithdrawal: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-withdrawal",
} as const;

export const simnet = {
  accounts,
  contracts,
  identifiers,
} as const;

export const deployments = {
  sbtcBootstrapSigners: {
    devnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-bootstrap-signers",
    simnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-bootstrap-signers",
    testnet: null,
    mainnet: null,
  },
  sbtcDeposit: {
    devnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-deposit",
    simnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-deposit",
    testnet: null,
    mainnet: null,
  },
  sbtcRegistry: {
    devnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-registry",
    simnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-registry",
    testnet: null,
    mainnet: null,
  },
  sbtcToken: {
    devnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-token",
    simnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-token",
    testnet: null,
    mainnet: null,
  },
  sbtcWithdrawal: {
    devnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-withdrawal",
    simnet: "ST2SBXRBJJTH7GV5J93HJ62W2NRRQ46XYBK92Y039.sbtc-withdrawal",
    testnet: null,
    mainnet: null,
  },
} as const;

export const project = {
  contracts,
  deployments,
} as const;
