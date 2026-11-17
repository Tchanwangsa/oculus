import { announceDriveChange, DRIVE_CHANGED_BACKEND_EVENT, type DriveChange } from "@/lib/drive/listing";
import { useTauriEvent } from "./useEvents";

/** A folder command's `drive-changed` becomes `DRIVE_CHANGED_EVENT` for each
 *  subject it touched, so every open listing and file list reloads, whoever
 *  called the command. */
export function useDriveEvents() {
  useTauriEvent<DriveChange>(DRIVE_CHANGED_BACKEND_EVENT, ({ payload }) => {
    if (!payload.subjectIds.length) return announceDriveChange();
    for (const id of payload.subjectIds) announceDriveChange(id);
  });
}
