import { Prop, Schema, SchemaFactory } from '@nestjs/mongoose';
import { Document } from 'mongoose';

export enum CredentialStatus {
  ACTIVE = 'active',
  REVOKED = 'revoked',
}

@Schema({ timestamps: true })
export class Credential extends Document {
  @Prop({ required: true })
  id: string;

  @Prop({ required: true })
  did: string;

  @Prop()
  substrateAddress: string;

  @Prop({ required: true, type: Object })
  credentialSubject: Record<string, any>;

  @Prop({ required: true, type: Object })
  credential: Record<string, any>;

  @Prop({ enum: CredentialStatus, default: CredentialStatus.ACTIVE })
  status: CredentialStatus;

  @Prop()
  expirationDate: Date;
}

export const CredentialSchema = SchemaFactory.createForClass(Credential);