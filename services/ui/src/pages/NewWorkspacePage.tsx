import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useState } from 'react';
import { apiPost } from '@/api/client';
import { ArrowLeft } from 'lucide-react';

export function NewWorkspacePage() {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [slug, setSlug] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      const ws = await apiPost<{ id: string }>('/workspaces/v1/workspaces', {
        name: name.trim(),
        description: description.trim() || undefined,
        slug: slug.trim() || undefined,
      });
      window.location.href = '/workspaces/' + ws.id;
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create workspace');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon" asChild>
          <Link to={"/workspaces" as any}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <h1 className="text-2xl font-bold">New Workspace</h1>
      </div>
      <Card className="max-w-md">
        <CardHeader><CardTitle>Create workspace</CardTitle></CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <Label htmlFor="name">Name</Label>
              <Input id="name" value={name} onChange={(e) => setName(e.target.value)} placeholder="My project" required />
            </div>
            <div>
              <Label htmlFor="description">Description (optional)</Label>
              <Input id="description" value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Brief description" />
            </div>
            <div>
              <Label htmlFor="slug">URL slug (optional)</Label>
              <Input id="slug" value={slug} onChange={(e) => setSlug(e.target.value)} placeholder="my-project" />
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            <Button type="submit" disabled={loading}>{loading ? 'Creating…' : 'Create'}</Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
