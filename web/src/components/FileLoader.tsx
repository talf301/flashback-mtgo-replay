/**
 * File Loader Component
 *
 * Handles loading replay files with drag-and-drop and file selection.
 * Supports JSON file parsing with validation and error handling.
 */

import { useState, useCallback, useRef } from 'react';
import type { ReplayFile } from '../types/replay';

export interface FileLoaderProps {
  onFileLoad: (file: ReplayFile) => void;
  onError?: (error: string) => void;
  className?: string;
  accept?: string;
  maxSizeBytes?: number;
}

export function FileLoader({
  onFileLoad,
  onError,
  className = '',
  accept = '.json,.flashback',
  maxSizeBytes = 10 * 1024 * 1024, // 10MB default
}: FileLoaderProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loadedFileName, setLoadedFileName] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const validateReplayFile = (data: unknown): data is ReplayFile => {
    if (!data || typeof data !== 'object') {
      throw new Error('Invalid file format: not an object');
    }

    const replay = data as Partial<ReplayFile>;

    if (!replay.header || typeof replay.header !== 'object') {
      throw new Error('Missing or invalid header');
    }

    if (typeof replay.header.game_id !== 'string') {
      throw new Error('Missing or invalid game_id in header');
    }

    if (!Array.isArray(replay.actions)) {
      throw new Error('Missing or invalid actions array');
    }

    return true;
  };

  const parseFile = useCallback(
    async (file: File): Promise<ReplayFile> => {
      setError(null);
      setIsLoading(true);

      try {
        // Check file size
        if (file.size > maxSizeBytes) {
          throw new Error(
            `File size (${(file.size / 1024 / 1024).toFixed(2)}MB) exceeds maximum (${maxSizeBytes / 1024 / 1024}MB)`,
          );
        }

        // Check file type
        if (accept && !file.name.endsWith('.json') && !file.name.endsWith('.flashback')) {
          throw new Error(`Invalid file type. Expected .json or .flashback`);
        }

        // Read file content
        const text = await file.text();
        let parsed: unknown;

        try {
          parsed = JSON.parse(text);
        } catch (parseError) {
          throw new Error('Failed to parse JSON file');
        }

        // Validate file structure
        if (!validateReplayFile(parsed)) {
          throw new Error('Invalid replay file structure');
        }

        setLoadedFileName(file.name);
        return parsed;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : 'Unknown error loading file';
        setError(errorMessage);
        onError?.(errorMessage);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [maxSizeBytes, accept, onError],
  );

  const handleFileSelect = useCallback(
    async (file: File) => {
      try {
        const replayFile = await parseFile(file);
        onFileLoad(replayFile);
      } catch {
        // Error already handled in parseFile
      }
    },
    [parseFile, onFileLoad],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);

      const files = Array.from(e.dataTransfer.files);
      if (files.length === 0) return;

      handleFileSelect(files[0]);
    },
    [handleFileSelect],
  );

  const handleDragOver = useCallback((e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  }, []);

  const handleInputChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (!files || files.length === 0) return;

      handleFileSelect(files[0]);
    },
    [handleFileSelect],
  );

  const handleButtonClick = () => {
    fileInputRef.current?.click();
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
  };

  return (
    <div className={`bg-slate-900/80 backdrop-blur border border-slate-700 rounded-lg p-6 ${className}`}>
      <h2 className="text-lg font-semibold text-white mb-4">Load Replay File</h2>

      {/* Drop zone */}
      <div
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onClick={handleButtonClick}
        className={`
          border-2 border-dashed rounded-lg p-8 text-center cursor-pointer transition-all
          ${isDragging ? 'border-blue-500 bg-blue-500/10' : 'border-slate-600 hover:border-slate-400 hover:bg-slate-800/50'}
          ${error ? 'border-red-500 bg-red-500/10' : ''}
        `}
      >
        <input
          ref={fileInputRef}
          type="file"
          accept={accept}
          onChange={handleInputChange}
          className="hidden"
        />

        {isLoading ? (
          <div className="flex flex-col items-center gap-3">
            <div className="w-12 h-12 border-4 border-slate-600 border-t-blue-500 rounded-full animate-spin"></div>
            <p className="text-slate-400">Loading file...</p>
          </div>
        ) : loadedFileName ? (
          <div className="flex flex-col items-center gap-3">
            <div className="w-16 h-16 bg-green-500/20 rounded-full flex items-center justify-center">
              <svg
                className="w-8 h-8 text-green-500"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M5 13l4 4L19 7"
                />
              </svg>
            </div>
            <div>
              <p className="text-white font-medium">{loadedFileName}</p>
              <p className="text-slate-400 text-sm">Click to load another file</p>
            </div>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-3">
            <div className="w-16 h-16 bg-slate-800 rounded-full flex items-center justify-center">
              <svg
                className="w-8 h-8 text-slate-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
                />
              </svg>
            </div>
            <div>
              <p className="text-white font-medium">
                Drag and drop replay file here
              </p>
              <p className="text-slate-400 text-sm">
                or click to browse
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Error message */}
      {error && (
        <div className="mt-4 p-3 bg-red-500/10 border border-red-500/50 rounded-lg">
          <p className="text-red-400 text-sm flex items-center gap-2">
            <svg
              className="w-4 h-4 flex-shrink-0"
              fill="currentColor"
              viewBox="0 0 20 20"
            >
              <path
                fillRule="evenodd"
                d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z"
                clipRule="evenodd"
              />
            </svg>
            {error}
          </p>
        </div>
      )}

      {/* File info */}
      {!error && !loadedFileName && (
        <div className="mt-4 p-3 bg-slate-800/50 rounded-lg">
          <p className="text-slate-400 text-xs">
            <span className="font-medium">Supported format:</span> JSON
          </p>
          <p className="text-slate-400 text-xs">
            <span className="font-medium">Maximum size:</span> {formatFileSize(maxSizeBytes)}
          </p>
        </div>
      )}

      {/* Reset button when file loaded */}
      {loadedFileName && !isLoading && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            setLoadedFileName(null);
            setError(null);
            if (fileInputRef.current) {
              fileInputRef.current.value = '';
            }
          }}
          className="mt-4 px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors w-full"
        >
          Clear File
        </button>
      )}
    </div>
  );
}
