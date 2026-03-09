/**
 * SCALE encoding utilities for reconstructing SignedBlock binary format
 * compatible with Substrate's `import-blocks` command.
 *
 * Binary format: each block is [4-byte LE length][SCALE-encoded SignedBlock]
 */

import type { BlockRow } from "../db/queries.ts";

/**
 * Encode a SCALE compact integer (unsigned).
 * Used for encoding lengths in SCALE format.
 */
export function encodeCompact(value: number): Buffer {
  if (value < 64) {
    // Single byte mode
    return Buffer.from([value << 2]);
  } else if (value < 16384) {
    // Two byte mode
    const v = (value << 2) | 0x01;
    return Buffer.from([v & 0xff, (v >> 8) & 0xff]);
  } else if (value < 1073741824) {
    // Four byte mode — use >>> 0 to force unsigned 32-bit to avoid sign overflow
    // for values between 2^29 and 2^30-1 where (value << 2) would flip the sign bit
    const v = ((value << 2) | 0x02) >>> 0;
    const buf = Buffer.alloc(4);
    buf.writeUInt32LE(v, 0);
    return buf;
  } else {
    // Big integer mode (5-byte form for values in [2^30, 2^32-1])
    if (value > 0xFFFFFFFF) {
      throw new Error(`encodeCompact: values > 2^32-1 not yet supported (got ${value})`);
    }
    const buf = Buffer.alloc(5);
    buf[0] = 0x03; // Big-integer flag (lower 2 bits = 11, upper 6 = byte count - 4)
    buf.writeUInt32LE(value, 1);
    return buf;
  }
}

/**
 * Encode a block number as a compact SCALE integer.
 */
function encodeBlockNumber(blockNumber: number): Buffer {
  // Block numbers in Substrate headers are encoded as compact<u32>
  return encodeCompact(blockNumber);
}

/**
 * Reconstruct a SCALE-encoded Header from stored components.
 *
 * Header layout:
 *   parent_hash: H256 (32 bytes, no length prefix)
 *   number: Compact<BlockNumber>
 *   state_root: H256 (32 bytes)
 *   extrinsics_root: H256 (32 bytes)
 *   digest: SCALE-encoded Digest (already complete)
 */
function encodeHeader(block: BlockRow): Buffer {
  return Buffer.concat([
    block.parent_hash,                    // 32 bytes
    encodeBlockNumber(block.block_number), // compact encoded
    block.state_root,                      // 32 bytes
    block.extrinsics_root,                 // 32 bytes
    block.digest,                          // SCALE-encoded Digest
  ]);
}

/**
 * Reconstruct a SCALE-encoded Block (Header + Vec<Extrinsic>).
 * The extrinsics are already stored as a SCALE-encoded Vec, so we just concatenate.
 */
function encodeBlock(block: BlockRow): Buffer {
  const header = encodeHeader(block);
  return Buffer.concat([header, block.extrinsics]);
}

/**
 * Reconstruct a SCALE-encoded SignedBlock (Block + Option<Justifications>).
 *
 * Option encoding:
 *   None: 0x00
 *   Some(x): 0x01 ++ x
 */
function encodeSignedBlock(block: BlockRow): Buffer {
  const blockData = encodeBlock(block);
  const justOpt = block.justifications
    ? Buffer.concat([Buffer.from([0x01]), block.justifications])
    : Buffer.from([0x00]);

  return Buffer.concat([blockData, justOpt]);
}

/**
 * Encode a single block for the import-blocks binary format:
 * [4-byte LE length][SCALE-encoded SignedBlock]
 */
export function encodeBlockForImport(block: BlockRow): Buffer {
  const signedBlock = encodeSignedBlock(block);
  const lengthPrefix = Buffer.alloc(4);
  lengthPrefix.writeUInt32LE(signedBlock.length, 0);
  return Buffer.concat([lengthPrefix, signedBlock]);
}
