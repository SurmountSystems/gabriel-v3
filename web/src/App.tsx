import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import BlocksGraph from './components/BlocksGraph';
import BlockStream from './components/BlockStream';
import './App.css';

const queryClient = new QueryClient();

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div className="container mx-auto p-4">
        <h1 className="text-2xl font-bold mb-4">Bitcoin P2PK Analysis</h1>
        <div className="grid gap-4">
          <BlocksGraph />
          <BlockStream />
        </div>
      </div>
    </QueryClientProvider>
  );
}

export default App;
