import { Link } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Shield, Key, FileCheck, Settings, ExternalLink, Lock } from 'lucide-react';

export function AccessManagement() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Access Management</h1>
        <p className="text-muted-foreground mt-1">
          GA4GH Passports, visas, and access policies for datasets and workflows.
        </p>
      </div>

      <div className="grid gap-6 md:grid-cols-2">
        <Card className="border-border/80">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Key className="h-4 w-4" />
              GA4GH Passports
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            <p className="text-sm text-muted-foreground">
              Passports encode visas (claims) for controlled access to data and services. This instance uses the
              GA4GH Passport standard for token-based authorization.
            </p>
            <p className="text-xs text-muted-foreground">
              Obtain a passport from your identity provider or visa issuer; the gateway validates JWTs and applies
              access rules to DRS, WES, and cohorts.
            </p>
          </CardContent>
        </Card>

        <Card className="border-border/80">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <FileCheck className="h-4 w-4" />
              Visa grants
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            <p className="text-sm text-muted-foreground">
              Visas are embedded in Passport tokens and describe what a user or process is allowed to access
              (e.g. a specific dataset, cohort, or workflow).
            </p>
            <p className="text-xs text-muted-foreground">
              Configure visa types and policies in your identity provider (e.g. Keycloak) or via admin APIs.
            </p>
          </CardContent>
        </Card>
      </div>

      <Card className="border-border/80">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Shield className="h-4 w-4" />
            What you can do here
          </CardTitle>
        </CardHeader>
        <CardContent>
          <ul className="space-y-3 text-sm">
            <li className="flex items-start gap-3">
              <Lock className="h-4 w-4 shrink-0 text-muted-foreground mt-0.5" />
              <span>
                <strong>Authentication:</strong> Use Keycloak (or another IdP) to sign in; the UI can store a
                Passport JWT for API calls. Set <code className="rounded bg-muted px-1">__ferrumPassport</code> or
                use the login flow if configured.
              </span>
            </li>
            <li className="flex items-start gap-3">
              <Settings className="h-4 w-4 shrink-0 text-muted-foreground mt-0.5" />
              <span>
                <strong>Configuration:</strong> Admins configure JWKS URL, required visas, and CORS in{' '}
                <Link to={"/settings" as any} className="text-primary hover:underline">
                  Settings
                </Link>
                . Security events and token revocation are available under the admin API.
              </span>
            </li>
            <li className="flex items-start gap-3">
              <ExternalLink className="h-4 w-4 shrink-0 text-muted-foreground mt-0.5" />
              <span>
                <strong>Documentation:</strong> See{' '}
                <a
                  href="https://github.com/ga4gh-duri/ga4gh-duri.github.io"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-primary hover:underline"
                >
                  GA4GH DURI (Passports)
                </a>{' '}
                for the Passport and Visa standard.
              </span>
            </li>
          </ul>
        </CardContent>
      </Card>

      <div className="flex flex-wrap gap-3">
        <Link
          to={"/settings" as any}
          className="inline-flex items-center gap-2 rounded-md border border-border bg-card px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted"
        >
          <Settings className="h-4 w-4" />
          Open Settings
        </Link>
        <Link
          to={"/data" as any}
          className="inline-flex items-center gap-2 rounded-md border border-border bg-card px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted"
        >
          <FileCheck className="h-4 w-4" />
          Data Browser (DRS)
        </Link>
      </div>
    </div>
  );
}
