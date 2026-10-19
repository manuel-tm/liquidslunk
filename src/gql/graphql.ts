/* eslint-disable */
/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends ' $fragmentName' | '__typename' ? T[P] : never };
import type { TypedDocumentNode as DocumentNode } from '@graphql-typed-document-node/core';
export type FileMoveInput = {
  fileId: string | number;
  path: string;
};

export type FileType =
  | 'image'
  | 'music'
  | 'other';

export type FileUseKind =
  | 'artist_link'
  | 'artist_picture'
  | 'label_picture'
  | 'release_cover'
  | 'release_link'
  | 'track_audio'
  | 'track_link';

export type LibraryCheckQueryVariables = Exact<{ [key: string]: never; }>;


export type LibraryCheckQuery = { libraryCheck: { fileCount: number, missingFiles: Array<{ movedTo: Array<string>, likelyMove: string | null, file: { id: string, path: string, type: FileType, uses: Array<{ kind: FileUseKind, entityId: string, name: string, note: string | null }> } }>, changedFiles: Array<{ sizeBytes: number, mtime: number, file: { id: string, path: string, sizeBytes: number, mtime: number } }>, unregisteredFolders: Array<{ path: string, files: Array<{ name: string, type: FileType }> }>, releasesWithoutArtists: Array<{ id: string, title: string }>, tracksWithoutPerformers: Array<{ id: string, title: string, cdNumber: number, trackNumber: number, release: { id: string, title: string } }> } };

export type RelinkFilesMutationVariables = Exact<{
  moves: Array<FileMoveInput> | FileMoveInput;
}>;


export type RelinkFilesMutation = { relinkFiles: Array<{ id: string, path: string }> };

export type RehashFilesMutationVariables = Exact<{
  ids: Array<string | number> | string | number;
}>;


export type RehashFilesMutation = { rehashFiles: Array<{ id: string, sizeBytes: number, mtime: number }> };


export const LibraryCheckDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"LibraryCheck"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"libraryCheck"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"fileCount"}},{"kind":"Field","name":{"kind":"Name","value":"missingFiles"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"file"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"path"}},{"kind":"Field","name":{"kind":"Name","value":"type"}},{"kind":"Field","name":{"kind":"Name","value":"uses"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"kind"}},{"kind":"Field","name":{"kind":"Name","value":"entityId"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"note"}}]}}]}},{"kind":"Field","name":{"kind":"Name","value":"movedTo"}},{"kind":"Field","name":{"kind":"Name","value":"likelyMove"}}]}},{"kind":"Field","name":{"kind":"Name","value":"changedFiles"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"file"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"path"}},{"kind":"Field","name":{"kind":"Name","value":"sizeBytes"}},{"kind":"Field","name":{"kind":"Name","value":"mtime"}}]}},{"kind":"Field","name":{"kind":"Name","value":"sizeBytes"}},{"kind":"Field","name":{"kind":"Name","value":"mtime"}}]}},{"kind":"Field","name":{"kind":"Name","value":"unregisteredFolders"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"path"}},{"kind":"Field","name":{"kind":"Name","value":"files"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"type"}}]}}]}},{"kind":"Field","name":{"kind":"Name","value":"releasesWithoutArtists"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"title"}}]}},{"kind":"Field","name":{"kind":"Name","value":"tracksWithoutPerformers"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"cdNumber"}},{"kind":"Field","name":{"kind":"Name","value":"trackNumber"}},{"kind":"Field","name":{"kind":"Name","value":"release"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"title"}}]}}]}}]}}]}}]} as unknown as DocumentNode<LibraryCheckQuery, LibraryCheckQueryVariables>;
export const RelinkFilesDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"RelinkFiles"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"moves"}},"type":{"kind":"NonNullType","type":{"kind":"ListType","type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"FileMoveInput"}}}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"relinkFiles"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"moves"},"value":{"kind":"Variable","name":{"kind":"Name","value":"moves"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"path"}}]}}]}}]} as unknown as DocumentNode<RelinkFilesMutation, RelinkFilesMutationVariables>;
export const RehashFilesDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"RehashFiles"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"ids"}},"type":{"kind":"NonNullType","type":{"kind":"ListType","type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"ID"}}}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"rehashFiles"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"ids"},"value":{"kind":"Variable","name":{"kind":"Name","value":"ids"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"sizeBytes"}},{"kind":"Field","name":{"kind":"Name","value":"mtime"}}]}}]}}]} as unknown as DocumentNode<RehashFilesMutation, RehashFilesMutationVariables>;