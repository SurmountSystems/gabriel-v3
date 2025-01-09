export interface BlockAggregate {
  date: string;
  block_height: number;
  block_hash_big_endian: string;
  total_p2pk_addresses: number;
  total_p2pk_value: number;
} 