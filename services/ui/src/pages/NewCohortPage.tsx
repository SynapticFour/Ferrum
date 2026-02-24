import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiPost } from '@/api/client';
import { ArrowLeft } from 'lucide-react';
import { Link } from '@tanstack/react-router';

const COHORTS_BASE = '/cohorts/v1';

type CreateResponse = {
  id: string;
  name: string;
  description: string | null;
  // ... other fields
};

export function NewCohortPage() {
  const navigate = useNavigate();
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const res = await apiPost<CreateResponse>(`${COHORTS_BASE}/cohorts`, {
        name: name || 'Unnamed cohort',
        description: description || null,
        tags: [],
        filter_criteria: {},
      });
      navigate({ to: '/cohorts/$cohortId', params: { cohortId: res.id } });
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link to="/cohorts">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <h1 className="text-3xl font-bold tracking-tight">New Cohort</h1>
      </div>
      <Card className="max-w-lg">
        <CardHeader>
          <CardTitle>Create cohort</CardTitle>
          <p className="text-sm text-muted-foreground">
            Create a named cohort to group samples and attach phenotype metadata.
          </p>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <Label htmlFor="name">Name</Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. BRCA Cohort 2024"
                className="mt-1"
              />
            </div>
            <div>
              <Label htmlFor="description">Description (optional)</Label>
              <Input
                id="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Short description"
                className="mt-1"
              />
            </div>
            {error && (
              <p className="text-sm text-destructive">{error}</p>
            )}
            <Button type="submit" disabled={submitting}>
              {submitting ? 'Creating…' : 'Create cohort'}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
