export { nativeVersion, newV4, newV4Bytes, newV7, newV7Bytes, newV7Batch, newV7BatchBytes, fillV7, MAX_V7_BATCH, Layout, Variant, NIL, MAX, version, variant, isRfc, getTimestamp, v7UnixMillis, v7Timestamp, newV5, v7ToSqlOrder, v7FromSqlOrder } from './binding'
export function isAvailable(): boolean
export const Namespaces: { readonly dns: string; readonly url: string; readonly oid: string; readonly x500: string }
export type HyperUuidErrorCode =
  | 'ERR_HYPERUUID_RANDOM_SOURCE'
  | 'ERR_HYPERUUID_TIMESTAMP_RANGE'
  | 'ERR_HYPERUUID_BATCH_TOO_LARGE'
  | 'ERR_HYPERUUID_WRONG_VERSION'
  | 'ERR_HYPERUUID_INVALID_ID'
  | 'ERR_HYPERUUID_INVALID_BUFFER'
  | 'ERR_HYPERUUID_INVALID_COUNT'
  | 'ERR_HYPERUUID_COUNTER_EXHAUSTED'
  | 'ERR_HYPERUUID_UNAVAILABLE'
export interface HyperUuidError extends Error {
  code: HyperUuidErrorCode
}
export function isHyperUuidError(value: unknown): value is HyperUuidError
