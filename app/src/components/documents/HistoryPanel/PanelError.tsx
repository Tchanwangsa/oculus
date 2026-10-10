import { Alert, AlertDescription } from "@/components/ui/alert";

export function PanelError({ message }: { message: string }) {
  return (
    <Alert variant="destructive" className="mx-3 my-2 w-auto px-2.5 py-2">
      <AlertDescription className="text-[11px] leading-snug break-words">{message}</AlertDescription>
    </Alert>
  );
}
