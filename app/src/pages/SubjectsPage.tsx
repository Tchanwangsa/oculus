import { BookOpen, FileText, Presentation, ClipboardList, Megaphone, ChevronRight, Folder, FolderOpen } from "lucide-react";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

const COURSES = [
  {
    code: "COMP30023",
    name: "Computer Systems",
    color: "#5da0ff",
    sections: [
      { label: "Modules",       icon: Folder,         count: 12 },
      { label: "Lecture Slides",icon: Presentation,   count: 11 },
      { label: "Assignments",   icon: ClipboardList,  count: 3  },
      { label: "Notices",       icon: Megaphone,      count: 5  },
    ],
  },
  {
    code: "SWEN30006",
    name: "Software Modelling & Design",
    color: "#73c6c2",
    sections: [
      { label: "Modules",       icon: Folder,        count: 8 },
      { label: "Lecture Slides",icon: Presentation,  count: 8 },
      { label: "Assignments",   icon: ClipboardList, count: 3 },
      { label: "Notices",       icon: Megaphone,     count: 2 },
    ],
  },
  {
    code: "COMP30027",
    name: "Machine Learning",
    color: "#a78bfa",
    sections: [
      { label: "Modules",       icon: Folder,        count: 10 },
      { label: "Lecture Slides",icon: Presentation,  count: 9  },
      { label: "Assignments",   icon: ClipboardList, count: 4  },
      { label: "Notices",       icon: Megaphone,     count: 3  },
    ],
  },
];

type File = { name: string; size: string; type: string };

const SAMPLE_FILES: File[] = [
  { name: "Week1_NetworkLayers.pdf",    size: "4.2 MB", type: "pdf" },
  { name: "Week2_TCPIP.pdf",            size: "3.8 MB", type: "pdf" },
  { name: "Week3_Routing.pdf",          size: "5.1 MB", type: "pdf" },
  { name: "Assignment1_Brief.pdf",      size: "1.2 MB", type: "pdf" },
  { name: "Week1_Module.md",            size: "18 KB",  type: "md"  },
  { name: "Week2_Module.md",            size: "22 KB",  type: "md"  },
];

export default function SubjectsPage() {
  const [selectedCourse, setSelectedCourse] = useState(COURSES[0].code);
  const [selectedSection, setSelectedSection] = useState("Lecture Slides");

  const course = COURSES.find((c) => c.code === selectedCourse)!;

  return (
    <div className="flex h-full">
      {/* Course list */}
      <div className="w-56 shrink-0 border-r border-border flex flex-col overflow-hidden">
        <div className="px-4 h-14 flex items-center border-b border-border shrink-0">
          <span className="font-semibold text-sm text-foreground">Subjects</span>
        </div>
        <div className="flex-1 overflow-y-auto py-2">
          {COURSES.map((c) => (
            <button
              key={c.code}
              onClick={() => { setSelectedCourse(c.code); setSelectedSection("Modules"); }}
              className={cn(
                "w-full text-left px-3 py-2.5 mx-1 rounded-lg flex items-center gap-3 transition-colors",
                selectedCourse === c.code
                  ? "bg-surface-raised text-foreground"
                  : "text-muted-foreground hover:bg-surface hover:text-foreground"
              )}
              style={{ width: "calc(100% - 8px)" }}
            >
              <div
                className="w-2 h-2 rounded-full shrink-0"
                style={{ backgroundColor: c.color }}
              />
              <div className="min-w-0">
                <p className="text-xs font-semibold truncate">{c.code}</p>
                <p className="text-[11px] text-muted-foreground truncate">{c.name}</p>
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Section nav + file browser */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Course header */}
        <div className="px-6 h-14 flex items-center gap-3 border-b border-border shrink-0">
          <BookOpen size={16} className="text-primary" />
          <div>
            <span className="font-semibold text-sm text-foreground">{course.code}</span>
            <span className="text-sm text-muted-foreground ml-2">{course.name}</span>
          </div>
        </div>

        <div className="flex flex-1 overflow-hidden">
          {/* Section list */}
          <div className="w-44 shrink-0 border-r border-border py-3 overflow-y-auto">
            {course.sections.map(({ label, icon: Icon, count }) => (
              <button
                key={label}
                onClick={() => setSelectedSection(label)}
                className={cn(
                  "w-full flex items-center justify-between gap-2 px-4 py-2.5 text-xs transition-colors",
                  selectedSection === label
                    ? "text-primary font-semibold bg-surface-raised"
                    : "text-muted-foreground hover:text-foreground hover:bg-surface"
                )}
              >
                <span className="flex items-center gap-2">
                  <Icon size={13} />
                  {label}
                </span>
                <Badge variant="secondary">{count}</Badge>
              </button>
            ))}
          </div>

          {/* File list */}
          <div className="flex-1 overflow-y-auto">
            <div className="px-6 py-4">
              <div className="flex items-center gap-2 mb-4">
                <FolderOpen size={14} className="text-muted-foreground" />
                <span className="text-xs font-medium text-muted-foreground">
                  {course.code} / {selectedSection}
                </span>
              </div>
              <div className="space-y-1">
                {SAMPLE_FILES.map((file) => (
                  <div
                    key={file.name}
                    className="flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-surface transition-colors cursor-pointer group"
                  >
                    <FileText size={14} className="text-muted-foreground shrink-0" />
                    <span className="text-xs text-foreground flex-1 truncate">{file.name}</span>
                    <span className="text-[11px] text-muted-foreground">{file.size}</span>
                    <span className="text-[10px] uppercase text-muted-foreground font-mono bg-surface-raised px-1.5 py-0.5 rounded">
                      {file.type}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
