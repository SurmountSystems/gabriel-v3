import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend } from 'recharts';
import { API_ENDPOINTS } from '../config/api';

interface BlockAggregate {
  date: string;
  block_height: number;
  block_hash_big_endian: string;
  total_utxos: number;
  total_sats: number;
}

function BlocksGraph() {
  const { data, isLoading, error } = useQuery({
    queryKey: ['aggregates'],
    queryFn: async () => {
      try {
        const response = await axios.get<BlockAggregate[]>(API_ENDPOINTS.latestBlocks);
        return response.data;
      } catch (error) {
        console.error('Error fetching data:', error);
        return [];
      }
    },
  });

  if (isLoading) return <div>Loading...</div>;
  if (error) return <div>Error loading data</div>;

  return (
    <div className="relative w-full">
      <h2 className="text-xl font-bold mb-4">P2PK Analysis Over Time</h2>
      <LineChart width={800} height={400} data={data} margin={{ top: 20, right: 100, bottom: 90, left: 50 }}>
        <CartesianGrid strokeDasharray="3 3" />
        <XAxis 
          dataKey="date" 
          tickFormatter={(timeStr) => {
            const date = new Date(timeStr);
            return date.toISOString().split('T')[0];
          }}
          angle={-90}
          textAnchor="end"
          height={60}
          label={{ 
            value: "Block Date", 
            position: "bottom", 
            offset: 70
          }}
        />
        <YAxis 
          yAxisId="left"
          label={{ 
            value: "Number of UTXOs", 
            angle: -90, 
            position: "insideLeft",
            offset: -35
          }}
        />
        <YAxis 
          yAxisId="right" 
          orientation="right"
          tickFormatter={(value) => (value / 100000000).toLocaleString(undefined, {
            minimumFractionDigits: 2,
            maximumFractionDigits: 2
          })}
        />
        <Tooltip 
          formatter={(value: number, name: string) => [
            name === "Total Value (BTC)" ? 
              (value / 100000000).toLocaleString(undefined, {
                minimumFractionDigits: 2,
                maximumFractionDigits: 2
              }) : value,
            name
          ]}
          labelFormatter={(timeStr) => {
            const date = new Date(timeStr);
            return date.toISOString().split('T')[0];
          }}
        />
        <Legend 
          verticalAlign="bottom" 
          height={36}
          wrapperStyle={{
            bottom: "15px",
            position: "relative"
          }}
        />
        <Line
          yAxisId="left"
          type="monotone"
          dataKey="total_utxos"
          stroke="#8884d8"
          name="Total UTXOs"
        />
        <Line
          yAxisId="right"
          type="monotone"
          dataKey="total_sats"
          stroke="#2e7d32"
          name="Total Value (BTC)"
        />
        <text
          x={780}
          y={200}
          textAnchor="middle"
          transform="rotate(90, 780, 200)"
          style={{ fontSize: '12px' }}
        >
          Total Value (BTC)
        </text>
      </LineChart>
    </div>
  );
}

export default BlocksGraph; 