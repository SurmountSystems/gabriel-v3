import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend } from 'recharts';

interface BlockAggregate {
  date: string;
  block_height: number;
  block_hash_big_endian: string;
  total_p2pk_addresses: number;
  total_p2pk_value: number;
}

function BlocksGraph() {
  const { data, isLoading, error } = useQuery({
    queryKey: ['aggregates'],
    queryFn: async () => {
      const response = await axios.get<BlockAggregate[]>('/api/aggregates');
      return response.data;
    },
  });

  if (isLoading) return <div>Loading...</div>;
  if (error) return <div>Error loading data</div>;

  return (
    <div>
      <h2 className="text-xl font-bold mb-4">P2PK Analysis Over Time</h2>
      <LineChart width={800} height={400} data={data}>
        <CartesianGrid strokeDasharray="3 3" />
        <XAxis dataKey="date" />
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

export default BlocksGraph; 