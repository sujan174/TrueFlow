"use client"

import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Badge } from "@/components/ui/badge"
import { Shield, Cloud, Server, Check, Loader2, ArrowLeft, ArrowRight, AlertCircle } from "lucide-react"
import { cn } from "@/lib/utils"
import { toast } from "sonner"

type VaultType = "aws_secrets_manager" | "aws_kms" | "hashicorp_vault" | "hashicorp_vault_kv" | "azure_key_vault"
type WizardStep = "select" | "configure" | "test" | "complete"

interface AwsSecretsManagerConfig {
  region: string
  assume_role_arn: string
  external_id: string
}

interface AwsKmsConfig {
  key_arn: string
  region: string
  assume_role_arn: string
  external_id: string
}

interface HashiCorpConfig {
  address: string
  mount_path: string
  namespace: string
  auth_method: "approle" | "kubernetes"
  approle_role_id: string
  approle_secret_id: string
  k8s_role: string
  k8s_jwt_path: string
}

interface HashiCorpKvConfig {
  address: string
  mount_path: string
  namespace: string
  auth_method: "approle" | "kubernetes"
  approle_role_id: string
  approle_secret_id: string
  k8s_role: string
  k8s_jwt_path: string
}

interface AzureKeyVaultConfig {
  vault_url: string
  tenant_id: string
  client_id: string
  client_secret: string
  use_managed_identity: boolean
  managed_identity_client_id: string
}

interface VaultConfigWizardProps {
  onComplete: () => void
  onCancel: () => void
}

export function VaultConfigWizard({ onComplete, onCancel }: VaultConfigWizardProps) {
  const [step, setStep] = useState<WizardStep>("select")
  const [vaultType, setVaultType] = useState<VaultType | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [testResult, setTestResult] = useState<"success" | "failure" | null>(null)

  // AWS Secrets Manager config
  const [awsSmConfig, setAwsSmConfig] = useState({
    region: "us-east-1",
    assume_role_arn: "",
    external_id: "",
  })

  // AWS KMS config
  const [awsKmsConfig, setAwsKmsConfig] = useState({
    key_arn: "",
    region: "us-east-1",
    assume_role_arn: "",
    external_id: "",
  })

  // HashiCorp Vault config
  const [hcConfig, setHcConfig] = useState({
    address: "",
    mount_path: "transit",
    namespace: "",
    auth_method: "approle" as "approle" | "kubernetes",
    approle_role_id: "",
    approle_secret_id: "",
    k8s_role: "",
    k8s_jwt_path: "/var/run/secrets/kubernetes.io/serviceaccount/token",
  })

  // HashiCorp Vault KV config
  const [hcKvConfig, setHcKvConfig] = useState({
    address: "",
    mount_path: "secret",
    namespace: "",
    auth_method: "approle" as "approle" | "kubernetes",
    approle_role_id: "",
    approle_secret_id: "",
    k8s_role: "",
    k8s_jwt_path: "/var/run/secrets/kubernetes.io/serviceaccount/token",
  })

  // Azure Key Vault config
  const [azureKvConfig, setAzureKvConfig] = useState({
    vault_url: "",
    tenant_id: "",
    client_id: "",
    client_secret: "",
    use_managed_identity: false,
    managed_identity_client_id: "",
  })

  const vaultOptions = [
    {
      type: "aws_secrets_manager" as VaultType,
      name: "AWS Secrets Manager",
      description: "Fetch secrets at runtime from AWS Secrets Manager",
      icon: Cloud,
      recommended: true,
    },
    {
      type: "aws_kms" as VaultType,
      name: "AWS KMS",
      description: "Decrypt pre-encrypted secrets with AWS KMS",
      icon: Shield,
      recommended: false,
    },
    {
      type: "hashicorp_vault" as VaultType,
      name: "HashiCorp Vault (Transit)",
      description: "Transit secrets engine for encryption/decryption",
      icon: Server,
      recommended: false,
    },
    {
      type: "hashicorp_vault_kv" as VaultType,
      name: "HashiCorp Vault KV",
      description: "KV v2 secrets engine for storing secrets",
      icon: Server,
      recommended: false,
    },
    {
      type: "azure_key_vault" as VaultType,
      name: "Azure Key Vault",
      description: "Microsoft Azure Key Vault for secrets management",
      icon: Cloud,
      recommended: false,
    },
  ]

  const handleTestConnection = async () => {
    setIsSubmitting(true)
    setTestResult(null)

    const getConfig = () => {
      switch (vaultType) {
        case "aws_secrets_manager":
          return awsSmConfig
        case "aws_kms":
          return awsKmsConfig
        case "hashicorp_vault":
          return hcConfig
        case "hashicorp_vault_kv":
          return hcKvConfig
        case "azure_key_vault":
          return azureKvConfig
        default:
          return {}
      }
    }

    try {
      const response = await fetch("/api/gateway/vault/test", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          backend: vaultType,
          config: getConfig(),
        }),
      })

      setTestResult(response.ok ? "success" : "failure")
      if (response.ok) {
        toast.success("Connection successful!")
      } else {
        toast.error("Connection failed. Check your configuration.")
      }
    } catch (error) {
      setTestResult("failure")
      toast.error("Failed to test connection")
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleSave = async () => {
    setIsSubmitting(true)

    const getConfig = () => {
      switch (vaultType) {
        case "aws_secrets_manager":
          return awsSmConfig
        case "aws_kms":
          return awsKmsConfig
        case "hashicorp_vault":
          return hcConfig
        case "hashicorp_vault_kv":
          return hcKvConfig
        case "azure_key_vault":
          return azureKvConfig
        default:
          return {}
      }
    }

    try {
      const response = await fetch("/api/gateway/vault/config", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          backend: vaultType,
          config: getConfig(),
        }),
      })

      if (response.ok) {
        setStep("complete")
        toast.success("Vault configuration saved!")
      } else {
        toast.error("Failed to save configuration")
      }
    } catch (error) {
      toast.error("Failed to save configuration")
    } finally {
      setIsSubmitting(false)
    }
  }

  const renderStepContent = () => {
    switch (step) {
      case "select":
        return (
          <div className="space-y-4">
            <div className="text-center mb-6">
              <h3 className="text-lg font-medium">Select Vault Type</h3>
              <p className="text-sm text-muted-foreground">
                Choose how you want to manage your API keys
              </p>
            </div>

            <div className="grid gap-3">
              {vaultOptions.map((option) => (
                <button
                  key={option.type}
                  onClick={() => {
                    setVaultType(option.type)
                    setStep("configure")
                  }}
                  className={cn(
                    "flex items-start gap-4 p-4 border rounded-lg text-left transition-all",
                    "hover:border-primary hover:bg-primary/5"
                  )}
                >
                  <option.icon className="h-6 w-6 text-muted-foreground shrink-0" />
                  <div className="flex-1">
                    <div className="flex items-center gap-2">
                      <span className="font-medium">{option.name}</span>
                      {option.recommended && (
                        <Badge variant="default" className="text-xs">Recommended</Badge>
                      )}
                    </div>
                    <p className="text-sm text-muted-foreground mt-1">{option.description}</p>
                  </div>
                </button>
              ))}
            </div>
          </div>
        )

      case "configure":
        return (
          <div className="space-y-4">
            <button
              onClick={() => setStep("select")}
              className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
            >
              <ArrowLeft className="h-4 w-4" />
              Back to selection
            </button>

            <div className="text-center mb-6">
              <h3 className="text-lg font-medium">Configure {vaultType === "aws_secrets_manager" ? "AWS Secrets Manager" : vaultType === "aws_kms" ? "AWS KMS" : vaultType === "hashicorp_vault" ? "HashiCorp Vault (Transit)" : vaultType === "hashicorp_vault_kv" ? "HashiCorp Vault KV" : "Azure Key Vault"}</h3>
              <p className="text-sm text-muted-foreground">
                Enter your vault credentials
              </p>
            </div>

            {vaultType === "aws_secrets_manager" && (
              <AwsSecretsManagerForm config={awsSmConfig} onChange={setAwsSmConfig} />
            )}

            {vaultType === "aws_kms" && (
              <AwsKmsForm config={awsKmsConfig} onChange={setAwsKmsConfig} />
            )}

            {vaultType === "hashicorp_vault" && (
              <HashiCorpForm config={hcConfig} onChange={setHcConfig} />
            )}

            {vaultType === "hashicorp_vault_kv" && (
              <HashiCorpKvForm config={hcKvConfig} onChange={setHcKvConfig} />
            )}

            {vaultType === "azure_key_vault" && (
              <AzureKeyVaultForm config={azureKvConfig} onChange={setAzureKvConfig} />
            )}

            <div className="flex justify-between pt-4">
              <Button variant="outline" onClick={onCancel}>
                Cancel
              </Button>
              <div className="flex gap-2">
                <Button variant="outline" onClick={handleTestConnection} disabled={isSubmitting}>
                  {isSubmitting ? <Loader2 className="h-4 w-4 animate-spin mr-2" /> : null}
                  Test Connection
                </Button>
                <Button onClick={() => setStep("test")} disabled={testResult !== "success"}>
                  Continue
                  <ArrowRight className="h-4 w-4 ml-2" />
                </Button>
              </div>
            </div>
          </div>
        )

      case "test":
        return (
          <div className="space-y-4">
            <div className="text-center mb-6">
              <h3 className="text-lg font-medium">Review & Save</h3>
              <p className="text-sm text-muted-foreground">
                Confirm your vault configuration
              </p>
            </div>

            <div className="bg-green-50 dark:bg-green-950/20 border border-green-200 dark:border-green-800 rounded-lg p-4">
              <div className="flex items-start gap-3">
                <Check className="h-5 w-5 text-green-600 dark:text-green-400 shrink-0 mt-0.5" />
                <div className="text-sm text-green-800 dark:text-green-200">
                  <p className="font-medium">Connection Test Passed</p>
                  <p className="mt-1">Your vault is properly configured and accessible.</p>
                </div>
              </div>
            </div>

            <div className="flex justify-between pt-4">
              <Button variant="outline" onClick={() => setStep("configure")}>
                <ArrowLeft className="h-4 w-4 mr-2" />
                Back
              </Button>
              <Button onClick={handleSave} disabled={isSubmitting}>
                {isSubmitting ? <Loader2 className="h-4 w-4 animate-spin mr-2" /> : null}
                Save Configuration
              </Button>
            </div>
          </div>
        )

      case "complete":
        return (
          <div className="text-center space-y-4">
            <div className="mx-auto w-12 h-12 bg-green-100 dark:bg-green-900 rounded-full flex items-center justify-center">
              <Check className="h-6 w-6 text-green-600 dark:text-green-400" />
            </div>
            <h3 className="text-lg font-medium">Vault Configured!</h3>
            <p className="text-sm text-muted-foreground">
              You can now create credentials using this vault backend.
            </p>
            <Button onClick={onComplete}>
              Done
            </Button>
          </div>
        )
    }
  }

  return (
    <Card className="max-w-2xl mx-auto">
      <CardHeader>
        <CardTitle>Add Vault Backend</CardTitle>
        <CardDescription>
          Configure an external vault for customer-managed keys
        </CardDescription>
      </CardHeader>
      <CardContent>
        {renderStepContent()}
      </CardContent>
    </Card>
  )
}

// Form components for each vault type
function AwsSecretsManagerForm({ config, onChange }: { config: AwsSecretsManagerConfig; onChange: (c: AwsSecretsManagerConfig) => void }) {
  return (
    <div className="space-y-4">
      <div className="bg-blue-50 dark:bg-blue-950/20 border border-blue-200 dark:border-blue-800 rounded-lg p-3">
        <div className="flex items-start gap-2">
          <AlertCircle className="h-4 w-4 text-blue-600 dark:text-blue-400 mt-0.5" />
          <div className="text-xs text-blue-800 dark:text-blue-200">
            <p className="font-medium">How it works</p>
            <p className="mt-1">
              Store your API keys in AWS Secrets Manager. AILink fetches them at request time.
              Your keys never enter AILink&apos;s database.
            </p>
          </div>
        </div>
      </div>

      <div>
        <Label htmlFor="region">AWS Region</Label>
        <Select value={config.region} onValueChange={(v) => v && onChange({ ...config, region: v })}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="us-east-1">US East (N. Virginia)</SelectItem>
            <SelectItem value="us-east-2">US East (Ohio)</SelectItem>
            <SelectItem value="us-west-2">US West (Oregon)</SelectItem>
            <SelectItem value="eu-west-1">EU (Ireland)</SelectItem>
            <SelectItem value="ap-northeast-1">Asia Pacific (Tokyo)</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div>
        <Label htmlFor="assume-role">Assume Role ARN (Optional)</Label>
        <Input
          id="assume-role"
          placeholder="arn:aws:iam::123456789012:role/AILinkSecretAccess"
          value={config.assume_role_arn}
          onChange={(e) => onChange({ ...config, assume_role_arn: e.target.value })}
        />
        <p className="text-xs text-muted-foreground mt-1">
          For cross-account secret access
        </p>
      </div>

      <div>
        <Label htmlFor="external-id">External ID (Optional)</Label>
        <Input
          id="external-id"
          placeholder="External ID for assume role"
          value={config.external_id}
          onChange={(e) => onChange({ ...config, external_id: e.target.value })}
        />
      </div>
    </div>
  )
}

function AwsKmsForm({ config, onChange }: { config: AwsKmsConfig; onChange: (c: AwsKmsConfig) => void }) {
  return (
    <div className="space-y-4">
      <div className="bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-800 rounded-lg p-3">
        <div className="flex items-start gap-2">
          <AlertCircle className="h-4 w-4 text-amber-600 dark:text-amber-400 mt-0.5" />
          <div className="text-xs text-amber-800 dark:text-amber-200">
            <p className="font-medium">Pre-encryption required</p>
            <p className="mt-1">
              With AWS KMS, you must pre-encrypt your API keys using the AWS CLI.
              Consider AWS Secrets Manager for simpler key management.
            </p>
          </div>
        </div>
      </div>

      <div>
        <Label htmlFor="key-arn">KMS Key ARN *</Label>
        <Input
          id="key-arn"
          placeholder="arn:aws:kms:us-east-1:123456789012:key/..."
          value={config.key_arn}
          onChange={(e) => onChange({ ...config, key_arn: e.target.value })}
          required
        />
      </div>

      <div>
        <Label htmlFor="region">AWS Region</Label>
        <Select value={config.region} onValueChange={(v) => v && onChange({ ...config, region: v })}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="us-east-1">US East (N. Virginia)</SelectItem>
            <SelectItem value="us-west-2">US West (Oregon)</SelectItem>
            <SelectItem value="eu-west-1">EU (Ireland)</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div>
        <Label htmlFor="assume-role">Assume Role ARN (Optional)</Label>
        <Input
          id="assume-role"
          placeholder="arn:aws:iam::123456789012:role/AILinkKMSAccess"
          value={config.assume_role_arn}
          onChange={(e) => onChange({ ...config, assume_role_arn: e.target.value })}
        />
      </div>
    </div>
  )
}

function HashiCorpForm({ config, onChange }: { config: HashiCorpConfig; onChange: (c: HashiCorpConfig) => void }) {
  return (
    <div className="space-y-4">
      <div className="bg-purple-50 dark:bg-purple-950/20 border border-purple-200 dark:border-purple-800 rounded-lg p-3">
        <div className="flex items-start gap-2">
          <AlertCircle className="h-4 w-4 text-purple-600 dark:text-purple-400 mt-0.5" />
          <div className="text-xs text-purple-800 dark:text-purple-200">
            <p className="font-medium">Transit Secrets Engine</p>
            <p className="mt-1">
              Configure HashiCorp Vault&apos;s Transit engine for encryption/decryption.
            </p>
          </div>
        </div>
      </div>

      <div>
        <Label htmlFor="address">Vault Address *</Label>
        <Input
          id="address"
          placeholder="https://vault.your-company.com:8200"
          value={config.address}
          onChange={(e) => onChange({ ...config, address: e.target.value })}
          required
        />
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <Label htmlFor="mount-path">Transit Mount Path</Label>
          <Input
            id="mount-path"
            placeholder="transit"
            value={config.mount_path}
            onChange={(e) => onChange({ ...config, mount_path: e.target.value })}
          />
        </div>
        <div>
          <Label htmlFor="namespace">Namespace (Enterprise)</Label>
          <Input
            id="namespace"
            placeholder="optional"
            value={config.namespace}
            onChange={(e) => onChange({ ...config, namespace: e.target.value })}
          />
        </div>
      </div>

      <div>
        <Label>Authentication Method</Label>
        <div className="grid grid-cols-2 gap-2 mt-2">
          <button
            type="button"
            onClick={() => onChange({ ...config, auth_method: "approle" })}
            className={cn(
              "p-3 border rounded-lg text-left transition-all",
              config.auth_method === "approle" ? "border-primary bg-primary/5" : ""
            )}
          >
            <span className="text-sm font-medium">AppRole</span>
            <p className="text-xs text-muted-foreground">Recommended for production</p>
          </button>
          <button
            type="button"
            onClick={() => onChange({ ...config, auth_method: "kubernetes" })}
            className={cn(
              "p-3 border rounded-lg text-left transition-all",
              config.auth_method === "kubernetes" ? "border-primary bg-primary/5" : ""
            )}
          >
            <span className="text-sm font-medium">Kubernetes</span>
            <p className="text-xs text-muted-foreground">For K8s deployments</p>
          </button>
        </div>
      </div>

      {config.auth_method === "approle" && (
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Label htmlFor="role-id">Role ID</Label>
            <Input
              id="role-id"
              placeholder="role-uuid"
              value={config.approle_role_id}
              onChange={(e) => onChange({ ...config, approle_role_id: e.target.value })}
            />
          </div>
          <div>
            <Label htmlFor="secret-id">Secret ID</Label>
            <Input
              id="secret-id"
              type="password"
              placeholder="secret-uuid"
              value={config.approle_secret_id}
              onChange={(e) => onChange({ ...config, approle_secret_id: e.target.value })}
            />
          </div>
        </div>
      )}

      {config.auth_method === "kubernetes" && (
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Label htmlFor="k8s-role">Kubernetes Role</Label>
            <Input
              id="k8s-role"
              placeholder="ailink-vault-role"
              value={config.k8s_role}
              onChange={(e) => onChange({ ...config, k8s_role: e.target.value })}
            />
          </div>
          <div>
            <Label htmlFor="k8s-jwt-path">JWT Path</Label>
            <Input
              id="k8s-jwt-path"
              placeholder="/var/run/secrets/kubernetes.io/serviceaccount/token"
              value={config.k8s_jwt_path}
              onChange={(e) => onChange({ ...config, k8s_jwt_path: e.target.value })}
            />
          </div>
        </div>
      )}
    </div>
  )
}

function HashiCorpKvForm({ config, onChange }: { config: HashiCorpKvConfig; onChange: (c: HashiCorpKvConfig) => void }) {
  return (
    <div className="space-y-4">
      <div className="bg-indigo-50 dark:bg-indigo-950/20 border border-indigo-200 dark:border-indigo-800 rounded-lg p-3">
        <div className="flex items-start gap-2">
          <AlertCircle className="h-4 w-4 text-indigo-600 dark:text-indigo-400 mt-0.5" />
          <div className="text-xs text-indigo-800 dark:text-indigo-200">
            <p className="font-medium">KV v2 Secrets Engine</p>
            <p className="mt-1">
              Store and retrieve secrets from HashiCorp Vault&apos;s KV v2 engine. Secrets are fetched at request time.
            </p>
          </div>
        </div>
      </div>

      <div>
        <Label htmlFor="hc-kv-address">Vault Address *</Label>
        <Input
          id="hc-kv-address"
          placeholder="https://vault.your-company.com:8200"
          value={config.address}
          onChange={(e) => onChange({ ...config, address: e.target.value })}
          required
        />
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <Label htmlFor="hc-kv-mount-path">KV Mount Path</Label>
          <Input
            id="hc-kv-mount-path"
            placeholder="secret"
            value={config.mount_path}
            onChange={(e) => onChange({ ...config, mount_path: e.target.value })}
          />
        </div>
        <div>
          <Label htmlFor="hc-kv-namespace">Namespace (Enterprise)</Label>
          <Input
            id="hc-kv-namespace"
            placeholder="optional"
            value={config.namespace}
            onChange={(e) => onChange({ ...config, namespace: e.target.value })}
          />
        </div>
      </div>

      <div>
        <Label>Authentication Method</Label>
        <div className="grid grid-cols-2 gap-2 mt-2">
          <button
            type="button"
            onClick={() => onChange({ ...config, auth_method: "approle" })}
            className={cn(
              "p-3 border rounded-lg text-left transition-all",
              config.auth_method === "approle" ? "border-primary bg-primary/5" : ""
            )}
          >
            <span className="text-sm font-medium">AppRole</span>
            <p className="text-xs text-muted-foreground">Recommended for production</p>
          </button>
          <button
            type="button"
            onClick={() => onChange({ ...config, auth_method: "kubernetes" })}
            className={cn(
              "p-3 border rounded-lg text-left transition-all",
              config.auth_method === "kubernetes" ? "border-primary bg-primary/5" : ""
            )}
          >
            <span className="text-sm font-medium">Kubernetes</span>
            <p className="text-xs text-muted-foreground">For K8s deployments</p>
          </button>
        </div>
      </div>

      {config.auth_method === "approle" && (
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Label htmlFor="hc-kv-role-id">Role ID</Label>
            <Input
              id="hc-kv-role-id"
              placeholder="role-uuid"
              value={config.approle_role_id}
              onChange={(e) => onChange({ ...config, approle_role_id: e.target.value })}
            />
          </div>
          <div>
            <Label htmlFor="hc-kv-secret-id">Secret ID</Label>
            <Input
              id="hc-kv-secret-id"
              type="password"
              placeholder="secret-uuid"
              value={config.approle_secret_id}
              onChange={(e) => onChange({ ...config, approle_secret_id: e.target.value })}
            />
          </div>
        </div>
      )}

      {config.auth_method === "kubernetes" && (
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Label htmlFor="hc-kv-k8s-role">Kubernetes Role</Label>
            <Input
              id="hc-kv-k8s-role"
              placeholder="ailink-vault-role"
              value={config.k8s_role}
              onChange={(e) => onChange({ ...config, k8s_role: e.target.value })}
            />
          </div>
          <div>
            <Label htmlFor="hc-kv-jwt-path">JWT Path</Label>
            <Input
              id="hc-kv-jwt-path"
              placeholder="/var/run/secrets/kubernetes.io/serviceaccount/token"
              value={config.k8s_jwt_path}
              onChange={(e) => onChange({ ...config, k8s_jwt_path: e.target.value })}
            />
          </div>
        </div>
      )}
    </div>
  )
}

function AzureKeyVaultForm({ config, onChange }: { config: AzureKeyVaultConfig; onChange: (c: AzureKeyVaultConfig) => void }) {
  return (
    <div className="space-y-4">
      <div className="bg-sky-50 dark:bg-sky-950/20 border border-sky-200 dark:border-sky-800 rounded-lg p-3">
        <div className="flex items-start gap-2">
          <AlertCircle className="h-4 w-4 text-sky-600 dark:text-sky-400 mt-0.5" />
          <div className="text-xs text-sky-800 dark:text-sky-200">
            <p className="font-medium">Azure Key Vault</p>
            <p className="mt-1">
              Fetch secrets from Azure Key Vault at request time. Supports both service principal and managed identity authentication.
            </p>
          </div>
        </div>
      </div>

      <div>
        <Label htmlFor="azure-vault-url">Key Vault URL *</Label>
        <Input
          id="azure-vault-url"
          placeholder="https://my-vault.vault.azure.net/"
          value={config.vault_url}
          onChange={(e) => onChange({ ...config, vault_url: e.target.value })}
          required
        />
      </div>

      <div className="flex items-center space-x-2">
        <input
          type="checkbox"
          id="use-managed-identity"
          checked={config.use_managed_identity}
          onChange={(e) => onChange({ ...config, use_managed_identity: e.target.checked })}
          className="h-4 w-4 rounded border-gray-300"
        />
        <Label htmlFor="use-managed-identity" className="text-sm font-normal">
          Use Managed Identity (for Azure deployments)
        </Label>
      </div>

      {config.use_managed_identity ? (
        <div>
          <Label htmlFor="azure-msi-client-id">User-Assigned Identity Client ID (Optional)</Label>
          <Input
            id="azure-msi-client-id"
            placeholder="Leave empty for system-assigned identity"
            value={config.managed_identity_client_id}
            onChange={(e) => onChange({ ...config, managed_identity_client_id: e.target.value })}
          />
          <p className="text-xs text-muted-foreground mt-1">
            Provide if using a user-assigned managed identity
          </p>
        </div>
      ) : (
        <>
          <div>
            <Label htmlFor="azure-tenant-id">Tenant ID *</Label>
            <Input
              id="azure-tenant-id"
              placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
              value={config.tenant_id}
              onChange={(e) => onChange({ ...config, tenant_id: e.target.value })}
              required
            />
          </div>

          <div>
            <Label htmlFor="azure-client-id">Client ID *</Label>
            <Input
              id="azure-client-id"
              placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
              value={config.client_id}
              onChange={(e) => onChange({ ...config, client_id: e.target.value })}
              required
            />
          </div>

          <div>
            <Label htmlFor="azure-client-secret">Client Secret *</Label>
            <Input
              id="azure-client-secret"
              type="password"
              placeholder="Azure AD app client secret"
              value={config.client_secret}
              onChange={(e) => onChange({ ...config, client_secret: e.target.value })}
              required
            />
          </div>
        </>
      )}
    </div>
  )
}