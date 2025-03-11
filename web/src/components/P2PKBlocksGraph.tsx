import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend } from 'recharts';
import { API_ENDPOINTS } from '../config/api';

/**
 * Knowing the specific UTXOs is a thing we won't need until much later.
 * The business value of a chart that graphs unspent P2PK keys and how much bitcoin is locked up in them 
 * is that we get an "early warning" if a significant number of them are being spent.
 */

interface BlockAggregate {
  date: string;
  block_height: number;
  block_hash_big_endian: string;
  total_utxos: number;
  total_sats: number;
}

function P2PKBlocksGraph() {
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
    <div id="chart-container" className="relative w-full">
      <h2 className="text-xl font-bold mb-4">P2PK UTXO Aggregates Over Time</h2>
      <LineChart width={800} height={400} data={data} margin={{ top: 15, right: 100, bottom: 90, left: 50 }}>
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
            offset: 65
          }}
        />
        <YAxis 
          yAxisId="left"
        />
        <text
          x={90}
          y={230}
          textAnchor="middle"
          transform="rotate(-90, 20, 200)"
          style={{ fontSize: '15px', fill: '#8884d8' }}
        >
          Number of UTXOs
        </text>
        <YAxis 
          yAxisId="right" 
          orientation="right"
          tickFormatter={(value) => (value / 100000000).toLocaleString(undefined, {
            minimumFractionDigits: 2,
            maximumFractionDigits: 2
          })}
        />
        <Tooltip 
          formatter={(value: number, name: string, props: any) => {
            const formattedValue = name === "Total Value (BTC)" 
              ? (value / 100000000).toLocaleString(undefined, {
                  minimumFractionDigits: 2,
                  maximumFractionDigits: 2
                })
              : value;
            return [formattedValue, name];
          }}
          labelFormatter={(label, payload) => {
            if (payload && payload.length > 0) {
              const { block_height, date } = payload[0].payload;
              const formattedDate = new Date(date).toISOString().split('T')[0];
              return `Block Height: ${block_height}, Date: ${formattedDate}`;
            }
            return '';
          }}
          content={({ payload, label }) => {
            if (payload && payload.length) {
              const { block_height, date } = payload[0].payload;
              const formattedDate = new Date(date).toISOString().split('T')[0];
              return (
                <div className="custom-tooltip" style={{
                  background: 'linear-gradient(white, #fafafa)',
                  padding: '10px',
                  borderRadius: '5px',
                  boxShadow: '0 0 5px rgba(0, 0, 0, 0.1)',
                  color: '#333'
                }}>
                  <p>{`Block Height: ${block_height}`}</p>
                  <p>{`Date: ${formattedDate}`}</p>
                  {payload.map((entry, index) => (
                    <p key={`item-${index}`}>{`${entry.name}: ${entry.value}`}</p>
                  ))}
                </div>
              );
            }
            return null;
          }}
        />
        <Legend 
          verticalAlign="bottom" 
          height={36}
          wrapperStyle={{
            bottom: "25px",
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
          x={700}
          y={195}
          textAnchor="middle"
          transform="rotate(90, 780, 200)"
          style={{ fontSize: '15px', fill: '#2e7d32' }}
        >
          Total Value (BTC)
        </text>
      </LineChart>
    </div>
  );
}

export default P2PKBlocksGraph; 