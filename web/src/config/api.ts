// You could also add other API-related configuration here
export const API_ENDPOINTS = {
    latestBlocks: '/api/blocks/latest',
    blockByHash: (hash: string) => `/api/block/hash/${hash}`,
    blockByHeight: (height: number) => `/api/block/height/${height}`,
    blockStream: '/api/blocks/stream',
};


