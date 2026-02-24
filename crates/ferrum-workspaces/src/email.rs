//! Email sending for workspace invites. Optional SMTP implementation via feature "email".

use crate::error::Result;
use async_trait::async_trait;

/// Sends workspace invite emails. When not configured, invites are stored but not emailed.
#[async_trait]
pub trait EmailSender: Send + Sync {
    /// Send an invite email to `to_email` for workspace `workspace_name` with link `invite_url`.
    async fn send_invite(
        &self,
        to_email: &str,
        workspace_name: &str,
        invite_url: &str,
    ) -> Result<()>;
}

/// No-op sender when email is not configured.
pub struct NoopEmailSender;

#[async_trait]
impl EmailSender for NoopEmailSender {
    async fn send_invite(&self, _to_email: &str, _workspace_name: &str, _invite_url: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "email")]
mod smtp {
    use super::*;
    use lettre::{
        message::header::ContentType,
        transport::smtp::authentication::Credentials,
        AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    };

    /// SMTP implementation using lettre.
    pub struct SmtpEmailSender {
        mailer: AsyncSmtpTransport<Tokio1Executor>,
        from: String,
    }

    impl SmtpEmailSender {
        pub fn new(config: &ferrum_core::EmailConfig) -> Result<Self> {
            let creds = match (&config.smtp_username, &config.smtp_password) {
                (Some(u), Some(p)) => Some(Credentials::new(u.clone(), p.clone())),
                _ => None,
            };
            let mut mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
                .map_err(|e| WorkspaceError::Internal(anyhow::anyhow!("SMTP relay: {}", e)))?
                .port(config.smtp_port);
            if let Some(c) = creds {
                mailer = mailer.credentials(c);
            }
            let mailer = mailer.build();
            Ok(Self {
                mailer,
                from: config.smtp_from.clone(),
            })
        }
    }

    #[async_trait]
    impl EmailSender for SmtpEmailSender {
        async fn send_invite(
            &self,
            to_email: &str,
            workspace_name: &str,
            invite_url: &str,
        ) -> Result<()> {
            let subject = format!("You've been invited to workspace '{}' on Ferrum", workspace_name);
            let body = format!(
                "You have been invited to join the workspace \"{}\" on Ferrum.\n\n\
                 Accept the invite by visiting:\n{}\n\n\
                 This link expires in 7 days.",
                workspace_name, invite_url
            );
            let email = Message::builder()
                .from(self.from.parse().map_err(|e: lettre::address::AddressError| WorkspaceError::Internal(e.into()))?)
                .to(to_email.parse().map_err(|e: lettre::address::AddressError| WorkspaceError::Internal(e.into()))?)
                .subject(&subject)
                .header(ContentType::TEXT_PLAIN)
                .body(body)
                .map_err(|e| WorkspaceError::Internal(e.into()))?;
            self.mailer.send(email).await.map_err(|e| WorkspaceError::Internal(e.into()))?;
            Ok(())
        }
    }
}

#[cfg(feature = "email")]
pub use smtp::SmtpEmailSender;
