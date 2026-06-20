import { apiPostFormData } from '@/api/client';

/** Use chunked ingest for files above this size (reliable through nginx + browser). */
export const CHUNKED_UPLOAD_THRESHOLD_BYTES = 2 * 1024 * 1024;

/** Per-request chunk size (each POST must stay under gateway/nginx body limits). */
const CHUNK_SIZE_BYTES = 2 * 1024 * 1024;

export interface ChunkedUploadOptions {
  encrypt?: boolean;
  workspaceId?: string;
  clientRequestId: string;
  onProgress?: (completed: number, total: number) => void;
}

interface ChunkUploadResponse {
  upload_token: string;
  chunk_offset: number;
  completed_bytes: number;
  total_bytes: number;
  complete: boolean;
  object_id?: string;
  size?: number;
}

export interface ChunkedUploadResult {
  object_id: string;
  size: number;
}

export async function uploadFileInChunks(
  file: File,
  opts: ChunkedUploadOptions,
): Promise<ChunkedUploadResult> {
  const total = file.size;
  if (total <= 0) {
    throw new Error('empty file');
  }

  let uploadToken: string | undefined;
  let offset = 0;

  while (offset < total) {
    const end = Math.min(offset + CHUNK_SIZE_BYTES, total);
    const slice = file.slice(offset, end);
    const fd = new FormData();
    fd.append('file', slice, file.name || 'upload.bin');
    fd.append('total_bytes', String(total));
    fd.append('chunk_offset', String(offset));
    if (uploadToken) fd.append('upload_token', uploadToken);
    if (offset === 0) {
      fd.append('client_request_id', opts.clientRequestId);
    }
    if (opts.encrypt) fd.append('encrypt', 'true');
    if (opts.workspaceId && end >= total) {
      fd.append('workspace_id', opts.workspaceId);
    }

    const resp = await apiPostFormData<ChunkUploadResponse>('/api/v1/ingest/upload/chunk', fd);
    uploadToken = resp.upload_token;
    opts.onProgress?.(resp.completed_bytes, resp.total_bytes);

    if (resp.complete) {
      const objectId = resp.object_id;
      if (!objectId) throw new Error('chunked upload completed without object_id');
      return { object_id: objectId, size: resp.size ?? total };
    }

    offset = resp.completed_bytes;
    if (offset <= 0 || offset > total) {
      throw new Error(`invalid chunk progress at offset ${offset}`);
    }
  }

  throw new Error('chunked upload ended without completion');
}
