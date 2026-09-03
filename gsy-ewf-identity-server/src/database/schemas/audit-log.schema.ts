import { Prop, Schema, SchemaFactory } from '@nestjs/mongoose';
import { Document } from 'mongoose';

export enum AuditAction {
  DID_CREATED = 'DID_CREATED',
  DID_UPDATED = 'DID_UPDATED',
  LOGIN_ATTEMPT = 'LOGIN_ATTEMPT',
  LOGIN_SUCCESS = 'LOGIN_SUCCESS',
  LOGIN_FAILURE = 'LOGIN_FAILURE',
  CREDENTIAL_ISSUED = 'CREDENTIAL_ISSUED',
  CREDENTIAL_VERIFIED = 'CREDENTIAL_VERIFIED',
  CREDENTIAL_REVOKED = 'CREDENTIAL_REVOKED',
}

@Schema({ timestamps: true })
export class AuditLog extends Document {
  @Prop({ required: true, enum: AuditAction })
  action: AuditAction;

  @Prop({ required: true })
  did: string;

  @Prop()
  gsyDexAddress?: string;

  @Prop({ type: Object })
  metadata?: Record<string, any>;

  @Prop()
  ipAddress?: string;

  @Prop()
  userAgent?: string;

  @Prop({ default: true })
  success: boolean;
}

export const AuditLogSchema = SchemaFactory.createForClass(AuditLog);