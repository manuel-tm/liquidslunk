import { createBrowserRouter } from "react-router"

import { AppLayout } from "@/components/app-layout"
import { ArtistForm } from "@/routes/artists/form"
import { ArtistList } from "@/routes/artists/list"
import {
  ArtistAppearances,
  ArtistCredits,
  ArtistFiles,
  ArtistReleases,
} from "@/routes/artists/page"
import { Dashboard } from "@/routes/dashboard"
import { ExternalLibraryCreate } from "@/routes/external-libraries/form"
import { ExternalLibraryList } from "@/routes/external-libraries/list"
import {
  ExternalLibraryContents,
  ExternalLibraryOverview,
  ExternalLibrarySettings,
} from "@/routes/external-libraries/page"
import { RegisterFiles } from "@/routes/files/create"
import { FileList } from "@/routes/files/list"
import { FileRead } from "@/routes/files/read"
import { FileUpdate } from "@/routes/files/update"
import {
  ImportCredits,
  ImportFiles,
  ImportFolder,
  ImportNewRelease,
  ImportRelease,
  ImportTags,
  ImportTracks,
} from "@/routes/import/steps"
import { LabelForm } from "@/routes/labels/form"
import { LabelList } from "@/routes/labels/list"
import { LabelRead } from "@/routes/labels/read"
import { LISTS } from "@/routes/lists/config"
import { ListItemForm } from "@/routes/lists/form"
import { ListItemList } from "@/routes/lists/list"
import { ListItemRead } from "@/routes/lists/read"
import { Welcome } from "@/routes/library"
import { LibraryCheck } from "@/routes/library-check"
import { NotFound } from "@/routes/not-found"
import { ReleaseCreate, ReleaseEdit } from "@/routes/releases/edit"
import { ReleaseFiles } from "@/routes/releases/files"
import { ReleaseList } from "@/routes/releases/list"
import { ReleasePage } from "@/routes/releases/page"
import { TagForm } from "@/routes/tags/form"
import { TagList } from "@/routes/tags/list"
import { TagRead } from "@/routes/tags/read"
import { TrackForm } from "@/routes/tracks/form"
import { TrackList } from "@/routes/tracks/list"
import { TrackRead } from "@/routes/tracks/read"

// The app's routes, and Welcome (design §6.1), outside the app's layout
export const router = createBrowserRouter([
  { path: "library", element: <Welcome /> },
  {
    element: <AppLayout />,
    children: [
      { index: true, element: <Dashboard /> },
      {
        path: "files",
        children: [
          { index: true, element: <FileList /> },
          { path: "create", element: <RegisterFiles /> },
          { path: ":id", element: <FileRead /> },
          { path: ":id/update", element: <FileUpdate /> },
        ],
      },
      {
        path: "releases",
        children: [
          { index: true, element: <ReleaseList /> },
          { path: "create", element: <ReleaseCreate /> },
          { path: ":id", element: <ReleasePage /> },
          { path: ":id/files", element: <ReleaseFiles /> },
          { path: ":id/update", element: <ReleaseEdit tab="details" /> },
          { path: ":id/update/tracks", element: <ReleaseEdit tab="tracks" /> },
          { path: ":id/update/files", element: <ReleaseEdit tab="files" /> },
          {
            path: ":id/update/credits",
            element: <ReleaseEdit tab="credits" />,
          },
          { path: ":id/update/tags", element: <ReleaseEdit tab="tags" /> },
        ],
      },
      {
        path: "artists",
        children: [
          { index: true, element: <ArtistList /> },
          { path: "create", element: <ArtistForm /> },
          { path: ":id", element: <ArtistReleases /> },
          { path: ":id/appearances", element: <ArtistAppearances /> },
          { path: ":id/credits", element: <ArtistCredits /> },
          { path: ":id/files", element: <ArtistFiles /> },
          { path: ":id/update", element: <ArtistForm /> },
        ],
      },
      {
        path: "tracks",
        children: [
          { index: true, element: <TrackList /> },
          { path: "create", element: <TrackForm /> },
          { path: ":id", element: <TrackRead /> },
          { path: ":id/update", element: <TrackForm /> },
        ],
      },
      {
        path: "labels",
        children: [
          { index: true, element: <LabelList /> },
          { path: "create", element: <LabelForm /> },
          { path: ":id", element: <LabelRead /> },
          { path: ":id/update", element: <LabelForm /> },
        ],
      },
      {
        path: "tags",
        children: [
          { index: true, element: <TagList /> },
          { path: "create", element: <TagForm /> },
          { path: ":type/:id", element: <TagRead /> },
          { path: ":type/:id/update", element: <TagForm /> },
        ],
      },
      ...Object.values(LISTS).map(({ kind, path }) => ({
        path: path.slice(1),
        children: [
          { index: true, element: <ListItemList kind={kind} /> },
          { path: "create", element: <ListItemForm kind={kind} /> },
          { path: ":id", element: <ListItemRead kind={kind} /> },
          { path: ":id/update", element: <ListItemForm kind={kind} /> },
        ],
      })),
      {
        path: "import",
        children: [
          { index: true, element: <ImportFolder /> },
          { path: "release", element: <ImportNewRelease /> },
          { path: ":id/release", element: <ImportRelease /> },
          { path: ":id/tracks", element: <ImportTracks /> },
          { path: ":id/files", element: <ImportFiles /> },
          { path: ":id/credits", element: <ImportCredits /> },
          { path: ":id/tags", element: <ImportTags /> },
        ],
      },
      { path: "library-check", element: <LibraryCheck /> },
      {
        path: "external-libraries",
        children: [
          { index: true, element: <ExternalLibraryList /> },
          { path: "create", element: <ExternalLibraryCreate /> },
          { path: ":id", element: <ExternalLibraryOverview /> },
          { path: ":id/contents", element: <ExternalLibraryContents /> },
          { path: ":id/settings", element: <ExternalLibrarySettings /> },
        ],
      },
      { path: "*", element: <NotFound /> },
    ],
  },
])
