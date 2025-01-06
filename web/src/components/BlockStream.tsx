import { useEffect, useState } from 'react';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend } from 'recharts';

interface BlockAggregate {
  date: string;
  block_height: number;
  block_hash_big_endian: string;
  total_p2pk_addresses: number;
  total_p2pk_value: number;
}

function BlockStream() {
  const [blocks, setBlocks] = useState<BlockAggregate[]>([]);

  useEffect(() => {
    const eventSource = new EventSource('/api/blocks/stream');

    eventSource.onmessage = (event) => {
      const newBlock = JSON.parse(event.data) as BlockAggregate;
      setBlocks((prevBlocks) => [...prevBlocks, newBlock].slice(-50)); // Keep last 50 blocks
    };

    return () => {
      eventSource.close();
    };
  }, []);

  return (
    <div>
      <h2 className="text-xl font-bold mb-4">Live Block Stream</h2>
      <LineChart width={800} height={400} data={blocks}>
        <CartesianGrid strokeDasharray="3 3" />
        <XAxis dataKey="block_height" />
        <YAxis yAxisId="left" />
        <YAxis yAxisId="right" orientation="right" />
        <Tooltip />
        <Legend />
        <Line
          yAxisId="left"
          type="monotone"
          dataKey="total_p2pk_addresses"
          stroke="#8884d8"
          name="Total P2PK Addresses"
        />
        <Line
          yAxisId="right"
          type="monotone"
          dataKey="total_p2pk_value"
          stroke="#82ca9d"
          name="Total P2PK Value (BTC)"
        />
      </LineChart>
    </div>
  );
}

export default BlockStream; 