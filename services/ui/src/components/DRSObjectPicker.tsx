import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import type { DrsObject } from '@/api/types';

export function DRSObjectPicker({
  open,
  onClose,
  onSelect,
}: {
  open: boolean;
  onClose: () => void;
  onSelect: (obj: DrsObject) => void;
}) {
  void onSelect;
  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent onClose={onClose}>
        <DialogHeader>
          <DialogTitle>Select DRS object</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <Input placeholder="Search by name or ID..." />
          <p className="text-sm text-muted-foreground">Browse and select. When list is wired, onSelect will be called.</p>
          <Button variant="outline" onClick={onClose}>Cancel</Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
