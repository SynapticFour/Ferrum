import { ImportToDrsDialog, type ImportToDrsDialogProps } from '@/components/ImportToDrsDialog';

/** @deprecated Use ImportToDrsDialog */
export type AddDataDialogProps = ImportToDrsDialogProps & {
  defaultWorkspaceId?: string;
};

/** Back-compat wrapper — imports into DRS; optionally links to workspace. */
export function AddDataDialog({ defaultWorkspaceId, ...props }: AddDataDialogProps) {
  return (
    <ImportToDrsDialog
      {...props}
      linkToWorkspaceId={defaultWorkspaceId}
      triggerLabelKey="data.importToDrs"
    />
  );
}

export { ImportToDrsDialog };
