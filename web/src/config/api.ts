export const API_BASE_URL = process.env.REACT_APP_GABRIEL_API_BASE_URL || 'http://localhost:3000';

// You could also add other API-related configuration here
export const API_ENDPOINTS = {
    latestBlocks: `${API_BASE_URL}/api/blocks/latest`,
    blockByHash: (hash: string) => `${API_BASE_URL}/api/block/hash/${hash}`,
    blockByHeight: (height: number) => `${API_BASE_URL}/api/block/height/${height}`,
    blockStream: `${API_BASE_URL}/api/blocks/stream`,
}; 