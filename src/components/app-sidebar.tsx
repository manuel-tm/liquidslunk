import {
  AlbumIcon,
  AudioLinesIcon,
  CassetteTapeIcon,
  DiscIcon,
  FilesIcon,
  FolderInputIcon,
  LayersIcon,
  LayoutDashboardIcon,
  type LucideIcon,
  ShieldCheckIcon,
  TagsIcon,
  UsersIcon,
} from "lucide-react"
import { Link, useMatch } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { LibraryMenu } from "@/components/library-menu"
import { SidebarPlayer } from "@/components/player/sidebar-player"
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"

type Section = { title: string; to: string; icon: LucideIcon }

// A section appears once its phase is built
const sections: Section[] = [
  { title: "Dashboard", to: "/", icon: LayoutDashboardIcon },
  { title: "Artists", to: "/artists", icon: UsersIcon },
  { title: "Releases", to: "/releases", icon: AlbumIcon },
  { title: "Tracks", to: "/tracks", icon: AudioLinesIcon },
  { title: "Labels", to: "/labels", icon: DiscIcon },
  { title: "Files", to: "/files", icon: FilesIcon },
  { title: "Library check", to: "/library-check", icon: ShieldCheckIcon },
]

// The lists the user manages
const lists: Section[] = [
  { title: "Tags", to: "/tags", icon: TagsIcon },
  { title: "Release formats", to: "/release-formats", icon: LayersIcon },
  { title: "Release media", to: "/release-media", icon: CassetteTapeIcon },
]

export function AppSidebar() {
  return (
    <Sidebar>
      <SidebarHeader className="gap-3">
        <LibraryMenu />
        <SidebarPlayer />
        <ButtonLink to="/import" className="justify-start">
          <FolderInputIcon data-icon="inline-start" />
          Import
        </ButtonLink>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {sections.map((section) => (
                <SectionLink key={section.to} {...section} />
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
        <SidebarGroup>
          <SidebarGroupLabel>Lists</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {lists.map((section) => (
                <SectionLink key={section.to} {...section} />
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  )
}

function SectionLink({ title, to, icon: Icon }: Section) {
  // A section stays active on its entities' pages, e.g. /artists/12
  const active = useMatch({ path: to, end: to === "/" }) !== null

  return (
    <SidebarMenuItem>
      <SidebarMenuButton render={<Link to={to} />} isActive={active}>
        <Icon />
        <span>{title}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  )
}
